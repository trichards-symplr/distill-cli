//! # Distill CLI
//!
//! A tool for transcribing and summarizing audio files using AWS services.
//!
//! This is the main entry point for the Distill CLI application. The application flow is:
//! 1. Parse command-line arguments
//! 2. Load AWS configuration and application settings
//! 3. Select or validate the S3 bucket for file storage
//! 4. Upload the audio file to S3 with server-side encryption (AES-256)
//! 5. Transcribe the audio using Amazon Transcribe
//! 6. Summarize the transcription using Amazon Bedrock
//! 7. Process the output based on the selected output type
//! 8. Optionally delete the S3 object
//! 9. Optionally save the full transcript
//!
//! ## Modules
//! The application is organized into the following modules:
//! - `aws_utils`: Handles AWS configuration, S3 bucket operations, and region detection
//! - `transcribe`: Manages the audio transcription process using Amazon Transcribe
//! - `summarize`: Handles text summarization using Amazon Bedrock
//! - `output`: Provides functions for different output formats and notifications

mod aws_utils;
mod output;
mod summarize;
mod transcribe;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use clap::Parser;
use config::{Config, File as ConfigFile};
use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select};
use spinoff::{Spinner, spinners, Color};

#[derive(Debug, Parser)]
#[clap(
    about = "Distill CLI can summarize an audio file (e.g., a meeting) using Amazon Transcribe and Amazon Bedrock.\n\nNotes:\n- S3 objects are deleted by default!\n- Use --save-transcript to keep the full transcript.",
    after_help = "For supported languages, consult: https://docs.aws.amazon.com/transcribe/latest/dg/supported-languages.html"
)]
struct Opt {
    #[clap(short, long)]
    input_audio_file: String,

    #[clap(
        short,
        long,
        value_enum,
        default_value = "Terminal",
        ignore_case = true
    )]
    output_type: OutputType,

    #[clap(short, long, default_value = "summarized_output")]
    summary_file_name: String,

    #[clap(short, long, default_value = "en-US")]
    language_code: String,

    #[clap(short, long, default_value = "Y")]
    delete_s3_object: String,
    
    #[clap(short = 't', long, help = "Save the full transcript to a .trans file")]
    save_transcript: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum OutputType {
    Terminal,
    Text,
    Word,
    Markdown,
    Slack,
    SlackSplit,
    Teams,
    TeamsSplit,
}

/// Prompts the user to enter a title for the Teams card
///
/// # Returns
///
/// The title entered by the user or a default value
///
/// Displays a prompt for the user to enter a title, provides a default value,
/// and handles input errors by falling back to the default value.
fn get_teams_card_title() -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("📝 Enter a title for the Teams card:")
        .default("A meeting from today...".to_string())
        .interact_text()
        .unwrap_or_else(|_| "A meeting from today...".to_string())
}

/// Gets webhooks from settings and prompts for selection if multiple are defined
///
/// # Arguments
///
/// * `settings` - Application configuration containing the webhooks
/// * `service` - Service name ("teams" or "slack")
///
/// # Returns
///
/// A vector of indices of the selected webhooks
///
/// This function handles both legacy single webhook configurations and multiple webhook arrays.
/// If only one webhook is configured (either as a legacy webhook_endpoint or as a single entry
/// in the webhooks array), it will be used automatically without prompting the user.
/// If multiple webhooks are configured, a multi-select dialog is shown to let the user
/// choose which webhooks to use.
fn select_webhooks(settings: &Config, service: &str) -> Result<Vec<usize>> {
    // Try to get webhooks array first
    let webhooks_path = format!("{}.webhooks", service);
    let endpoint_path = format!("{}.webhook_endpoint", service);
    
    let webhooks = match settings.get_array(&webhooks_path) {
        Ok(webhooks) => webhooks,
        Err(_) => {
            // Check for legacy single webhook
            let webhook_endpoint = settings
                .get_string(&endpoint_path)
                .unwrap_or_default();
                
            if !webhook_endpoint.is_empty() {
                // If we have a single legacy webhook, return index 0
                return Ok(vec![0]);
            }
            return Ok(vec![]);
        }
    };
    
    if webhooks.is_empty() {
        return Ok(vec![]);
    }
    
    // If there's only one webhook, return it without prompting
    if webhooks.len() == 1 {
        return Ok(vec![0]);
    }
    
    // For multiple webhooks, show selection dialog
    let webhook_names: Vec<String> = webhooks
        .iter()
        .map(|w| {
            // Convert to table and extract name safely
            match w.clone().into_table() {
                Ok(table) => table.get("name")
                    .and_then(|v| v.clone().into_string().ok())
                    .unwrap_or_else(|| "Unnamed webhook".to_string()),
                Err(_) => "Invalid webhook".to_string()
            }
        })
        .collect();
    
    let prompt = format!("📝 Select {} channels to send the summary to", service);
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(&prompt)
        .items(&webhook_names)
        .interact()?;
    
    Ok(selections)
}

/// Gets Teams webhooks from settings and prompts for selection if multiple are defined
///
/// # Arguments
///
/// * `settings` - Application configuration containing the Teams webhooks
///
/// # Returns
///
/// A vector of indices of the selected webhooks
fn select_teams_webhooks(settings: &Config) -> Result<Vec<usize>> {
    select_webhooks(settings, "teams")
}

/// Gets Slack webhooks from settings and prompts for selection if multiple are defined
///
/// # Arguments
///
/// * `settings` - Application configuration containing the Slack webhooks
///
/// # Returns
///
/// A vector of indices of the selected webhooks
fn select_slack_webhooks(settings: &Config) -> Result<Vec<usize>> {
    select_webhooks(settings, "slack")
}

/// Selects or validates an S3 bucket for file storage
///
/// # Arguments
///
/// * `s3_client` - AWS S3 client instance
/// * `s3_bucket_name` - Optional preconfigured bucket name from settings
///
/// # Returns
///
/// A Result containing the selected bucket name or an error
///
/// Checks if the provided bucket name exists in the user's account.
/// If the bucket exists, uses it; otherwise shows a selection menu.
/// Returns an error if no valid bucket is found.
async fn select_bucket(s3_client: &Client, s3_bucket_name: &str) -> Result<String> {
    let resp = &aws_utils::list_buckets(s3_client).await;
    let mut bucket_name = String::new();

    if !s3_bucket_name.is_empty() {
        if resp
            .as_ref()
            .ok()
            .and_then(|buckets| buckets.iter().find(|b| b.as_str() == s3_bucket_name))
            .is_some()
        {
            println!("📦 S3 bucket name: {}", s3_bucket_name);
            bucket_name = s3_bucket_name.to_string();
        } else {
            println!(
                "Error: The configured S3 bucket '{}' was not found.",
                s3_bucket_name
            );
        }
    }

    if bucket_name.is_empty() {
        match resp {
            Ok(bucket_names) => {
                if bucket_names.is_empty() {
                    bail!("\nNo S3 buckets found. Please create an S3 bucket first.");
                }
                
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a destination S3 bucket for your audio file")
                    .default(0)
                    .items(&bucket_names[..])
                    .interact()?;

                bucket_name = bucket_names[selection].clone();
            }
            Err(err) => {
                println!("Error getting bucket list: {}", err);
                bail!("\nError getting bucket list: {}", err);
            }
        };
    }

    if bucket_name.is_empty() {
        bail!("\nNo valid S3 bucket found. Please check your AWS configuration.");
    }

    Ok(bucket_name)
}

/// Loads application settings from the config.toml file
///
/// # Returns
///
/// A Result containing the loaded configuration or an error
///
/// Attempts to load the config.toml file from the current directory
/// and parses it into a Config object.
fn load_settings() -> Result<Config> {
    Config::builder()
        .add_source(ConfigFile::with_name("./config.toml"))
        .build()
        .context("Failed to load config.toml. Make sure it exists in the current directory.")
}

/// Main entry point for the Distill CLI application
///
/// # Returns
///
/// A Result indicating success or an error
///
/// Parses command-line arguments, loads configurations, processes the audio file,
/// and handles the output based on user preferences.
#[::tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    // Reset the spinner stopped flag at the beginning of the application
    output::reset_spinner_flag();
    
    // Parse command-line arguments first
    let Opt {
        input_audio_file,
        output_type,
        summary_file_name,
        language_code,
        delete_s3_object,
        save_transcript,
    } = Opt::parse();
    
    // Display input file and output type at the beginning
    println!("🧙 Welcome to Distill CLI");
    
    // Extract just the filename without path
    let file_path = Path::new(&input_audio_file);
    let file_name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| input_audio_file.clone());
    
    println!("📄 Processing file: {}", file_name);
    println!("🔄 Output type: {:?}", output_type);
    
    // Load AWS config
    let config = aws_utils::load_config(None).await;
    
    // Load application settings from config.toml
    let settings = load_settings()?;

    let s3_bucket_name = settings
        .get_string("aws.s3_bucket_name")
        .unwrap_or_default();

    let model_id = settings
        .get_string("model.model_id")
        .unwrap_or_default();

    let s3_client = Client::new(&config);

    println!("📦 Using model: {}", model_id);

    // Select or validate S3 bucket
    let bucket_name = select_bucket(&s3_client, &s3_bucket_name).await?;

    if output_type != OutputType::Teams && output_type != OutputType::TeamsSplit && 
       output_type != OutputType::Slack && output_type != OutputType::SlackSplit {
        println!("📦 Current output file name: {}", summary_file_name);
    }

    // Get Teams card title if needed
    let user_input = if output_type == OutputType::Teams || output_type == OutputType::TeamsSplit {
        get_teams_card_title()
    } else {
        String::new()
    };
    
    // Select webhooks early if needed
    let slack_webhook_indices = if output_type == OutputType::Slack || output_type == OutputType::SlackSplit {
        select_slack_webhooks(&settings)?
    } else {
        vec![]
    };
    
    let teams_webhook_indices = if output_type == OutputType::Teams || output_type == OutputType::TeamsSplit {
        select_teams_webhooks(&settings)?
    } else {
        vec![]
    };
    
    // Check if we have webhooks selected when needed
    if (output_type == OutputType::Slack || output_type == OutputType::SlackSplit) && slack_webhook_indices.is_empty() {
        println!("⚠️ No Slack webhooks selected.");
    }
    
    if (output_type == OutputType::Teams || output_type == OutputType::TeamsSplit) && teams_webhook_indices.is_empty() {
        println!("⚠️ No Teams webhooks selected.");
    }

    let mut spinner = Spinner::new(spinners::Dots, "Uploading file to S3...", Color::White);

    // Load the bucket region and create a new client to use that region
    let region = aws_utils::bucket_region(&s3_client, &bucket_name).await?;
    println!();
    
    let region_message = format!("Using bucket region {}", region);
    let static_region_message: &'static str = Box::leak(region_message.into_boxed_str());
    spinner.update(spinners::Dots, static_region_message, Some(Color::White));

    let regional_config = aws_utils::load_config(Some(region)).await;
    let regional_s3_client = Client::new(&regional_config);

    // Handle conversion of relative paths to absolute paths
    let file_path = Path::new(&input_audio_file);

    let absolute_path = shellexpand::tilde(file_path.to_str().unwrap()).to_string();
    let absolute_path = Path::new(&absolute_path);

    if !absolute_path.exists() {
        bail!("\n❌ The path {} does not exist.", absolute_path.display());
    }

    let canonicalized_path = absolute_path.canonicalize()?;
    let body = ByteStream::from_path(&canonicalized_path)
        .await
        .with_context(|| format!("❌ Error loading file: {}", canonicalized_path.display()))?;

    let _upload_result = regional_s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&file_name)
        .body(body)
        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256)
        .send()
        .await
        .context("❌ Failed to upload to S3")?;

    let s3_uri = format!("s3://{}/{}", bucket_name, file_name);

    println!();
    spinner.update(spinners::Dots, "Transcribing audio...", Some(Color::White));

    // Transcribe the audio
    let transcription: String = transcribe::transcribe_audio(
        &regional_config,
        file_path,
        &s3_uri,
        &mut spinner,
        &language_code,
    )
    .await?;

    // Summarize the transcription
    spinner.update(spinners::Dots, "Summarizing text...", Some(Color::White));
    let summarized_text = summarize::summarize_text(&config, &transcription, &mut spinner).await?;

    // Process output based on selected output type
    match output_type {
        OutputType::Word => {
            output::write_word_file(&summary_file_name.clone(), &summarized_text, &mut spinner)?;
        }
        OutputType::Text => {
            output::write_text_file(&summary_file_name.clone(), &summarized_text, &mut spinner)?;
        }
        OutputType::Terminal => {
            if !output::SPINNER_STOPPED.load(std::sync::atomic::Ordering::SeqCst) {
                spinner.success("Done!");
                output::SPINNER_STOPPED.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            println!();
            println!("Summary:\n{}\n", summarized_text);
        }
        OutputType::Markdown => {
            output::write_markdown_file(&summary_file_name.clone(), &summarized_text, &mut spinner)?;
        }
        OutputType::Slack => {
            if slack_webhook_indices.is_empty() {
                println!("⚠️ No Slack webhooks selected. Displaying summary in terminal instead.");
                println!("Summary:\n{}\n", summarized_text);
            } else {
                output::send_slack_notification(
                    &settings,
                    &mut spinner,
                    &summarized_text,
                    &slack_webhook_indices,
                )
                .await?;
            }
        }
        OutputType::SlackSplit => {
            // First write to a file
            let ext: &str = ".txt";
            let outfile = summary_file_name.clone() + ext;
            let output_file_path_txt = Path::new(&outfile);
            let mut file = File::create(output_file_path_txt)
                .map_err(|e| anyhow::anyhow!("❌ Error creating file: {}", e))?;

            file.write_all(summarized_text.as_bytes())
                .map_err(|e| anyhow::anyhow!("❌ Error creating file: {}", e))?;

            println!("\n💾 Summary written to {}", output_file_path_txt.display());
            
            // Update spinner for Slack notification
            if !slack_webhook_indices.is_empty() {
                if !output::SPINNER_STOPPED.load(std::sync::atomic::Ordering::SeqCst) {
                    spinner.update(spinners::Dots, "Sending to Slack...", Some(Color::White));
                }
                
                output::send_slack_notification(
                    &settings,
                    &mut spinner,
                    &summarized_text,
                    &slack_webhook_indices,
                )
                .await?;
            } else {
                println!("⚠️ No Slack webhooks selected. Summary was only written to file.");
            }
        }
        OutputType::Teams => {
            if teams_webhook_indices.is_empty() {
                println!("⚠️ No Teams webhooks selected. Displaying summary in terminal instead.");
                println!("Summary:\n{}\n", summarized_text);
            } else {
                output::send_teams_notification(
                    &settings,
                    &mut spinner,
                    &summarized_text,
                    &user_input,
                    "Summary sent to Teams!",
                    &teams_webhook_indices,
                )
                .await?;
            }
        }
        OutputType::TeamsSplit => {
            // First write to a file
            let ext: &str = ".txt";
            let outfile = summary_file_name.clone() + ext;
            let output_file_path_txt = Path::new(&outfile);
            let mut file = File::create(output_file_path_txt)
                .map_err(|e| anyhow::anyhow!("❌ Error creating file: {}", e))?;

            file.write_all(summarized_text.as_bytes())
                .map_err(|e| anyhow::anyhow!("❌ Error creating file: {}", e))?;

            println!("\n💾 Summary written to {}", output_file_path_txt.display());
            
            // Update spinner for Teams notification
            if !teams_webhook_indices.is_empty() {
                if !output::SPINNER_STOPPED.load(std::sync::atomic::Ordering::SeqCst) {
                    spinner.update(spinners::Dots, "Sending to Teams...", Some(Color::White));
                }
                
                output::send_teams_notification(
                    &settings,
                    &mut spinner,
                    &summarized_text,
                    &user_input,
                    "Summary sent to Teams and written to output file!",
                    &teams_webhook_indices,
                )
                .await?;
            } else {
                println!("⚠️ No Teams webhooks selected. Summary was only written to file.");
            }
        }
    }

    // After processing, check if the user wants to delete the S3 object
    if delete_s3_object == "Y" {
        s3_client
            .delete_object()
            .bucket(&bucket_name)
            .key(&file_name)
            .send()
            .await?;
    }
    
    // Save transcript if requested (as the last operation)
    if save_transcript {
        let trans_ext = ".trans";
        let trans_file = summary_file_name.clone() + trans_ext;
        let trans_path = Path::new(&trans_file);
        let mut trans_file = File::create(trans_path)
            .map_err(|e| anyhow::anyhow!("❌ Error creating transcript file: {}", e))?;
            
        trans_file.write_all(transcription.as_bytes())
            .map_err(|e| anyhow::anyhow!("❌ Error writing transcript file: {}", e))?;
            
        println!("📝 Full transcript saved to {}", trans_path.display());
    }

    if !output::SPINNER_STOPPED.load(std::sync::atomic::Ordering::SeqCst) {
        spinner.success("Done!");
    } else {
        println!("Done!");
    }

    Ok(())
}
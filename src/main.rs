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
use dialoguer::{theme::ColorfulTheme, Input, Select};
use spinoff::{spinners, Color, Spinner};

#[derive(Debug, Parser)]
#[clap(
    about = "Distill CLI can summarize an audio file (e.g., a meeting) using Amazon Transcribe and Amazon Bedrock.",
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

/// Get Teams card title from user input
fn get_teams_card_title() -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("📝 Enter a title for the Teams card:")
        .default("A meeting from today...".to_string())
        .interact_text()
        .unwrap_or_else(|_| "A meeting from today...".to_string())
}

/// Select or validate S3 bucket
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

/// Load application settings from config.toml
fn load_settings() -> Result<Config> {
    Config::builder()
        .add_source(ConfigFile::with_name("./config.toml"))
        .build()
        .context("Failed to load config.toml. Make sure it exists in the current directory.")
}

#[::tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    // Parse command-line arguments first
    let Opt {
        input_audio_file,
        output_type,
        summary_file_name,
        language_code,
        delete_s3_object,
    } = Opt::parse();
    
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

    println!("🧙 Welcome to Distill CLI");
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

    let mut spinner = Spinner::new(spinners::Dots7, "Uploading file to S3...", Color::Green);

    // Load the bucket region and create a new client to use that region
    let region = aws_utils::bucket_region(&s3_client, &bucket_name).await?;
    println!();
    spinner.update(
        spinners::Dots7,
        format!("Using bucket region {}", region),
        None,
    );
    let regional_config = aws_utils::load_config(Some(region)).await;
    let regional_s3_client = Client::new(&regional_config);

    // Handle conversion of relative paths to absolute paths
    let file_path = Path::new(&input_audio_file);
    let file_name = file_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    println!();
    spinner.update(
        spinners::Dots7,
        format!("Audio File {}", file_name),
        None,
    );

    let absolute_path = shellexpand::tilde(file_path.to_str().unwrap()).to_string();
    let absolute_path = Path::new(&absolute_path);

    if !absolute_path.exists() {
        bail!("\nThe path {} does not exist.", absolute_path.display());
    }

    let canonicalized_path = absolute_path.canonicalize()?;
    let body = ByteStream::from_path(&canonicalized_path)
        .await
        .with_context(|| format!("Error loading file: {}", canonicalized_path.display()))?;

    let _upload_result = regional_s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&file_name)
        .body(body)
        .send()
        .await
        .context("Failed to upload to S3")?;

    let s3_uri = format!("s3://{}/{}", bucket_name, file_name);

    println!();
    spinner.update(spinners::Dots7, "Transcribing audio...", None);

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
    spinner.update(spinners::Dots7, "Summarizing text...", None);
    let summarized_text = summarize::summarize_text(&config, &transcription, &mut spinner).await?;

    // Process output based on selected output type
    match output_type {
        OutputType::Word => {
            output::write_word_file(&summary_file_name, &summarized_text, &mut spinner)?;
        }
        OutputType::Text => {
            output::write_text_file(&summary_file_name, &summarized_text, &mut spinner)?;
        }
        OutputType::Terminal => {
            spinner.success("Done!");
            println!();
            println!("Summary:\n{}\n", summarized_text);
        }
        OutputType::Markdown => {
            output::write_markdown_file(&summary_file_name, &summarized_text, &mut spinner)?;
        }
        OutputType::Slack => {
            output::send_slack_notification(&settings, &mut spinner, &summarized_text).await?;
        }
        OutputType::SlackSplit => {
            // First write to a file
            let ext: &str = ".txt";
            let outfile = summary_file_name + ext;
            let output_file_path_txt = Path::new(&outfile);
            let mut file = File::create(output_file_path_txt)
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            file.write_all(summarized_text.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            println!("\n💾 Summary written to {}", output_file_path_txt.display());
            
            // Update spinner for Slack notification
            spinner.update(spinners::Dots7, "Sending to Slack...", None);
            
            // Send to Slack
            output::send_slack_notification(&settings, &mut spinner, &summarized_text).await?;
        }
        OutputType::Teams => {
            output::send_teams_notification(
                &settings,
                &mut spinner,
                &summarized_text,
                &user_input,
                "Summary sent to Teams!",
            )
            .await?;
        }
        OutputType::TeamsSplit => {
            // First write to a file
            let ext: &str = ".txt";
            let outfile = summary_file_name + ext;
            let output_file_path_txt = Path::new(&outfile);
            let mut file = File::create(output_file_path_txt)
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            file.write_all(summarized_text.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

            println!("\n💾 Summary written to {}", output_file_path_txt.display());
            
            // Update spinner for Teams notification
            spinner.update(spinners::Dots7, "Sending to Teams...", None);
            
            // Send to Teams
            output::send_teams_notification(
                &settings,
                &mut spinner,
                &summarized_text,
                &user_input,
                "Summary sent to Teams and written to output file!",
            )
            .await?;
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

    Ok(())
}
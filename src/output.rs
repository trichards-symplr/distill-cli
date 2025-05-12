//! # Output Module
//!
//! This module handles all output operations for the Distill CLI application:
//! - Writing summaries to different file formats (text, Word, Markdown)
//! - Sending notifications to communication platforms (Slack, Teams)
//!
//! Each function in this module takes care of a specific output format or notification
//! channel, handling the formatting, file creation, or API communication as needed.
//! The module provides a consistent interface for the main application to use regardless
//! of the output destination.
//!
//! ## Webhook Support
//! 
//! This module supports sending notifications to both single and multiple webhooks:
//! - Legacy single webhook endpoints via `webhook_endpoint` config setting
//! - Multiple named webhooks via `webhooks` array in config
//! 
//! When multiple webhooks are configured, the user can select which ones to use
//! through a multi-select interface.
//!
//! ## Spinner Thread Management
//!
//! This module manages a global `SPINNER_STOPPED` flag to ensure that spinner threads
//! are only stopped once. The flag is reset at the beginning of the application with
//! `reset_spinner_flag()` and checked before any operation that would stop a spinner.
//! 
//! All functions that use spinners should:
//! 1. Check `SPINNER_STOPPED` before stopping a spinner
//! 2. Set `SPINNER_STOPPED` to true after stopping a spinner
//! 3. Use `spinner.update()` instead of `spinner.stop_and_persist()` when possible

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use config::Config;
use docx_rs::{Docx, Paragraph, Run};
use reqwest::Client as ReqwestClient;
use serde_json::json;
use spinoff::{Spinner, spinners, Color};

// Global flag to track whether the spinner has been stopped
pub static SPINNER_STOPPED: AtomicBool = AtomicBool::new(false);

/// Resets the spinner stopped flag
/// 
/// This function should be called at the beginning of the application
/// to reset the spinner stopped flag. This ensures that the spinner thread
/// can be properly managed throughout the application lifecycle.
/// 
/// # Usage
/// 
/// Call this function once at the start of the main function:
/// 
/// ```rust
/// fn main() {
///     output::reset_spinner_flag();
///     // Rest of the application...
/// }
/// ```
pub fn reset_spinner_flag() {
    SPINNER_STOPPED.store(false, Ordering::SeqCst);
}

/// Writes summary content to a text file
///
/// # Arguments
///
/// * `summary_file_name` - Base name for the output file (without extension)
/// * `summarized_text` - The text content to write to the file
/// * `spinner` - Progress spinner to update upon completion
///
/// # Returns
///
/// A Result indicating success or an error
///
/// Creates a text file with the provided name and .txt extension,
/// writes the summarized text to the file, and updates the spinner
/// to indicate completion.
pub fn write_text_file(summary_file_name: &str, summarized_text: &str, spinner: &mut Spinner) -> Result<()> {
    let ext = ".txt";
    let outfile = summary_file_name.to_owned() + ext;
    let output_file_path = Path::new(&outfile);
    let mut file = File::create(output_file_path)
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    file.write_all(summarized_text.as_bytes())
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    // Simply update the spinner with success message
    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
        spinner.success("Done!");
        SPINNER_STOPPED.store(true, Ordering::SeqCst);
    }
    
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Writes summary content to a Microsoft Word document
///
/// # Arguments
///
/// * `summary_file_name` - Base name for the output file (without extension)
/// * `summarized_text` - The text content to write to the file
/// * `spinner` - Progress spinner to update upon completion
///
/// # Returns
///
/// A Result indicating success or an error
///
/// Creates a Word document (.docx) with the provided name, formats the content
/// using docx-rs library, and adds the summarized text as paragraphs in the document.
pub fn write_word_file(summary_file_name: &str, summarized_text: &str, spinner: &mut Spinner) -> Result<()> {
    let ext = ".docx";
    let outfile = summary_file_name.to_owned() + ext;
    let output_file_path = Path::new(&outfile);
    let file = File::create(output_file_path)
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    // Creating a new document and adding paragraphs
    let doc = Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(summarized_text)))
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("\n\n")));

    // Building and saving the document
    doc.build()
        .pack(file)
        .map_err(|e| anyhow::anyhow!("Error writing Word document: {}", e))?;

    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
        spinner.success("Done!");
        SPINNER_STOPPED.store(true, Ordering::SeqCst);
    }
    
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Writes summary content to a Markdown file
///
/// # Arguments
///
/// * `summary_file_name` - Base name for the output file (without extension)
/// * `summarized_text` - The text content to write to the file
/// * `spinner` - Progress spinner to update upon completion
///
/// # Returns
///
/// A Result indicating success or an error
///
/// Creates a Markdown file with the provided name and .md extension,
/// formats the content with Markdown syntax, and adds a header.
pub fn write_markdown_file(summary_file_name: &str, summarized_text: &str, spinner: &mut Spinner) -> Result<()> {
    let ext = ".md";
    let outfile = summary_file_name.to_owned() + ext;
    let output_file_path = Path::new(&outfile);
    let mut file = File::create(output_file_path)
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    let summary_md = format!("# Summary\n\n{}", summarized_text);
    let markdown_content = format!("{}", summary_md);

    file.write_all(markdown_content.as_bytes())
        .map_err(|e| anyhow::anyhow!("Error writing Markdown file: {}", e))?;

    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
        spinner.success("Done!");
        SPINNER_STOPPED.store(true, Ordering::SeqCst);
    }
    
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Sends a summary notification to one or more Slack webhooks
///
/// # Arguments
///
/// * `settings` - Application configuration containing the Slack webhook URLs
/// * `spinner` - Progress spinner to update during the process
/// * `summarized_text` - The text content to send to Slack
/// * `webhook_indices` - Indices of the selected webhooks to use
///
/// # Returns
///
/// A Result indicating success or an error
///
/// # Spinner Management
///
/// This function checks the `SPINNER_STOPPED` flag before stopping the spinner
/// and updates the flag after stopping it. It uses `spinner.update()` for progress
/// updates and only stops the spinner at the end of processing.
///
/// # Webhook Processing
///
/// Retrieves the Slack webhooks from settings, formats the summary as a Slack message,
/// and sends the message to each selected Slack webhook endpoint. Supports both legacy
/// single webhook configuration and multiple webhook configuration.
pub async fn send_slack_notification(
    settings: &Config,
    spinner: &mut Spinner,
    summarized_text: &str,
    webhook_indices: &[usize],
) -> Result<()> {
    let client = ReqwestClient::new();

    // Get webhooks from config
    let webhooks = match settings.get_array("slack.webhooks") {
        Ok(webhooks) => webhooks,
        Err(_) => {
            // Try legacy single webhook endpoint for backward compatibility
            let slack_webhook_endpoint = settings
                .get_string("slack.webhook_endpoint")
                .unwrap_or_default();
                
            if slack_webhook_endpoint.is_empty() {
                if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                    spinner.stop_and_persist(
                        "⚠️",
                        "Slack webhook endpoint is not configured. Skipping Slack notification.",
                    );
                    SPINNER_STOPPED.store(true, Ordering::SeqCst);
                }
                println!("Summary:\n{}\n", summarized_text);
                return Ok(());
            }
            
            // For legacy single webhook, just use it directly
            let message = "Sending to Slack";
            spinner.update(spinners::Dots, message, Some(Color::White));
            
            let content = format!("A summarization job just completed:\n\n{}", summarized_text);
            let payload = json!({
                "content": content
            });
            
            let result = client
                .post(&slack_webhook_endpoint)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;
                
            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                            spinner.success("Summary sent to Slack!");
                            SPINNER_STOPPED.store(true, Ordering::SeqCst);
                        }
                    } else {
                        let status = response.status();
                        println!("❌ Error sending summary to Slack: {}", status);
                        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                            spinner.stop_and_persist("❌", "Failed to send summary to Slack!");
                            SPINNER_STOPPED.store(true, Ordering::SeqCst);
                        }
                    }
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    println!("❌ Error sending summary to Slack: {}", err_msg);
                    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                        spinner.stop_and_persist("❌", "Failed to send summary to Slack!");
                        SPINNER_STOPPED.store(true, Ordering::SeqCst);
                    }
                }
            }
            
            return Ok(());
        }
    };
    
    if webhooks.is_empty() || webhook_indices.is_empty() {
        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
            spinner.stop_and_persist(
                "⚠️",
                "No Slack webhooks selected. Skipping Slack notification.",
            );
            SPINNER_STOPPED.store(true, Ordering::SeqCst);
        }
        println!("Summary:\n{}\n", summarized_text);
        return Ok(());
    }
    
    // Create the message payload
    let content = format!("A summarization job just completed:\n\n{}", summarized_text);
    let payload = json!({
        "content": content
    });
    
    // Update the main spinner instead of stopping it
    let processing_msg = format!("Processing {} Slack webhooks...", webhook_indices.len());
    let static_processing_msg: &'static str = Box::leak(processing_msg.into_boxed_str());
    spinner.update(spinners::Dots, static_processing_msg, Some(Color::White));
    
    // Send to each selected webhook
    let mut success_count = 0;
    let mut failure_count = 0;
    
    for &index in webhook_indices {
        if index >= webhooks.len() {
            continue;
        }
        
        // Extract webhook details safely
        let webhook = &webhooks[index];
        
        // Use into_table() to get a table view of the config value
        let webhook_table = match webhook.clone().into_table() {
            Ok(table) => table,
            Err(_) => continue,
        };
        
        // Get name and endpoint from the table
        let webhook_name = webhook_table.get("name")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| format!("Webhook {}", index + 1));
            
        let endpoint = match webhook_table.get("endpoint").and_then(|v| v.clone().into_string().ok()) {
            Some(ep) => ep,
            None => continue,
        };
        
        if endpoint.is_empty() {
            continue;
        }
        
        // Update the main spinner with the current webhook
        let message = format!("Sending to Slack ({})", webhook_name);
        let static_message: &'static str = Box::leak(message.into_boxed_str());
        spinner.update(spinners::Dots, static_message, Some(Color::White));
        
        let result = client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;
            
        match result {
            Ok(response) => {
                if response.status().is_success() {
                    success_count += 1;
                    // Update spinner with success message
                    let success_msg = format!("Successfully sent to Slack ({})", webhook_name);
                    let static_success_msg: &'static str = Box::leak(success_msg.into_boxed_str());
                    spinner.update(spinners::Dots, static_success_msg, Some(Color::Green));
                } else {
                    let status = response.status();
                    // Update spinner with error message
                    let error_msg = format!("Error sending to Slack ({}): {}", webhook_name, status);
                    let static_error_msg: &'static str = Box::leak(error_msg.into_boxed_str());
                    spinner.update(spinners::Dots, static_error_msg, Some(Color::Red));
                    failure_count += 1;
                }
            }
            Err(err) => {
                let err_msg = err.to_string();
                // Update spinner with error message
                let error_msg = format!("Error sending to Slack ({}): {}", webhook_name, err_msg);
                let static_error_msg: &'static str = Box::leak(error_msg.into_boxed_str());
                spinner.update(spinners::Dots, static_error_msg, Some(Color::Red));
                failure_count += 1;
            }
        }
    }
    
    // Update the spinner with the final result
    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
        if failure_count == 0 && success_count > 0 {
            let message = format!("Summary sent to {} Slack webhooks", success_count);
            let static_message: &'static str = Box::leak(message.into_boxed_str());
            spinner.success(static_message);
        } else if failure_count > 0 && success_count > 0 {
            let message = format!("Sent to {} Slack webhooks, failed to send to {} webhooks", success_count, failure_count);
            let static_message: &'static str = Box::leak(message.into_boxed_str());
            spinner.stop_and_persist("⚠️", static_message);
        } else {
            spinner.stop_and_persist("❌", "Failed to send summary to any Slack webhooks!");
        }
        SPINNER_STOPPED.store(true, Ordering::SeqCst);
    }
    
    Ok(())
}

/// Sends a summary notification to one or more Microsoft Teams webhooks
///
/// # Arguments
///
/// * `settings` - Application configuration containing the Teams webhook URLs
/// * `spinner` - Progress spinner to update during the process
/// * `summarized_text` - The text content to send to Teams
/// * `user_input` - Title for the Teams card
/// * `success_message` - Message to display on successful delivery
/// * `webhook_indices` - Indices of the selected webhooks to use
///
/// # Returns
///
/// A Result indicating success or an error
///
/// # Spinner Management
///
/// This function checks the `SPINNER_STOPPED` flag before stopping the spinner
/// and updates the flag after stopping it. It uses `spinner.update()` for progress
/// updates and only stops the spinner at the end of processing.
///
/// # Webhook Processing
///
/// Retrieves the Teams webhooks from settings, creates an adaptive card with the summary content,
/// and sends the card to each selected Teams webhook endpoint. Supports both legacy
/// single webhook configuration and multiple webhook configuration.
pub async fn send_teams_notification(
    settings: &Config,
    spinner: &mut Spinner,
    summarized_text: &str,
    user_input: &str,
    success_message: &str,
    webhook_indices: &[usize],
) -> Result<()> {
    let client = ReqwestClient::new();
    
    // Get current date and format it
    let current_date = chrono::Local::now();
    let formatted_date = current_date.format("%m-%d-%Y %I:%M:%S %p").to_string();
    let tz = tz::TimeZone::local().expect("Unable to determine timezone");
    let tz_name = tz.find_current_local_time_type()
        .expect("Could not find local timezone type")
        .time_zone_designation();
    let date_header = format!("Date: {} {}", formatted_date, tz_name);

    // Create the adaptive card payload
    let text = format!("{}", summarized_text);
    let payload = json!({
        "type":"message",
        "attachments":[
           {
              "contentType":"application/vnd.microsoft.card.adaptive",
              "contentUrl":null,
              "content":{
                 "$schema":"http://adaptivecards.io/schemas/adaptive-card.json",
                 "type":"AdaptiveCard",
                 "version":"1.5",
                 "msteams": {
                    "width": "Full"
                  },
                 "body":[
                    {
                        "type": "ColumnSet",
                        "columns": [
                            {
                                "type": "Column",
                                "items": [
                                    {
                                        "type": "Icon",
                                        "name": "Flash",
                                        "size": "Large",
                                        "style": "Filled",
                                        "color": "Accent"
                                    },
                                ],
                                "width": "auto"
                            },
                            {
                                "type": "Column",
                                "spacing": "medium",
                                "verticalContentAlignment": "center",
                                "items": [
                                    {
                                        "type": "TextBlock",
                                        "wrap": true,
                                        "style": "heading",
                                        "weight": "Bolder",
                                        "size": "Large",
                                        "text": user_input
                                    },
                                ],
                                "width": "auto"
                            }
                        ]
                    },
                    {
                        "type": "TextBlock",
                        "wrap": true,
                        "style": "heading",
                        "weight": "Bolder",
                        "size": "Medium",
                        "text": date_header
                    },
                    {
                        "type": "Container",
                        "showBorder": true,
                        "roundedCorners": true,
                        "maxHeight": "400px",
                        "items": [
                            {
                                "type": "TextBlock",
                                "maxLines": 100,
                                "wrap": true,
                                "text": text
                            }
                        ]
                    }
                 ]
              }
           }
        ]
    });

    // Get webhooks from config
    let webhooks = match settings.get_array("teams.webhooks") {
        Ok(webhooks) => webhooks,
        Err(_) => {
            // Try legacy single webhook endpoint for backward compatibility
            let teams_webhook_endpoint = settings
                .get_string("teams.webhook_endpoint")
                .unwrap_or_default();
                
            if teams_webhook_endpoint.is_empty() {
                if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                    spinner.stop_and_persist(
                        "⚠️",
                        "Teams webhook endpoint is not configured. Skipping Teams notification.",
                    );
                    SPINNER_STOPPED.store(true, Ordering::SeqCst);
                }
                println!("Summary:\n{}\n", summarized_text);
                return Ok(());
            }
            
            // For legacy single webhook, just use it directly
            let message = "Sending to Teams";
            spinner.update(spinners::Dots, message, Some(Color::White));
            
            let result = client
                .post(&teams_webhook_endpoint)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await;
                
            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                            spinner.success(success_message);
                            SPINNER_STOPPED.store(true, Ordering::SeqCst);
                        }
                    } else {
                        let status = response.status();
                        println!("❌ Error sending summary to Teams: {}", status);
                        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                            spinner.stop_and_persist("❌", "Failed to send summary to Teams!");
                            SPINNER_STOPPED.store(true, Ordering::SeqCst);
                        }
                    }
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    println!("❌ Error sending summary to Teams: {}", err_msg);
                    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
                        spinner.stop_and_persist("❌", "Failed to send summary to Teams!");
                        SPINNER_STOPPED.store(true, Ordering::SeqCst);
                    }
                }
            }
            
            return Ok(());
        }
    };
    
    if webhooks.is_empty() || webhook_indices.is_empty() {
        if !SPINNER_STOPPED.load(Ordering::SeqCst) {
            spinner.stop_and_persist(
                "⚠️",
                "No Teams webhooks selected. Skipping Teams notification.",
            );
            SPINNER_STOPPED.store(true, Ordering::SeqCst);
        }
        println!("Summary:\n{}\n", summarized_text);
        return Ok(());
    }
    
    // Update the main spinner instead of stopping it
    let processing_msg = format!("Processing {} Teams webhooks...", webhook_indices.len());
    let static_processing_msg: &'static str = Box::leak(processing_msg.into_boxed_str());
    spinner.update(spinners::Dots, static_processing_msg, Some(Color::White));
    
    // Send to each selected webhook
    let mut success_count = 0;
    let mut failure_count = 0;
    
    for &index in webhook_indices {
        if index >= webhooks.len() {
            continue;
        }
        
        // Extract webhook details safely
        let webhook = &webhooks[index];
        
        // Use into_table() to get a table view of the config value
        let webhook_table = match webhook.clone().into_table() {
            Ok(table) => table,
            Err(_) => continue,
        };
        
        // Get name and endpoint from the table
        let webhook_name = webhook_table.get("name")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| format!("Webhook {}", index + 1));
            
        let endpoint = match webhook_table.get("endpoint").and_then(|v| v.clone().into_string().ok()) {
            Some(ep) => ep,
            None => continue,
        };
        
        if endpoint.is_empty() {
            continue;
        }
        
        // Update the main spinner with the current webhook
        let message = format!("Sending to Teams ({})", webhook_name);
        let static_message: &'static str = Box::leak(message.into_boxed_str());
        spinner.update(spinners::Dots, static_message, Some(Color::White));
        
        let result = client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;
            
        match result {
            Ok(response) => {
                if response.status().is_success() {
                    success_count += 1;
                    // Update spinner with success message
                    let success_msg = format!("Successfully sent to Teams ({})", webhook_name);
                    let static_success_msg: &'static str = Box::leak(success_msg.into_boxed_str());
                    spinner.update(spinners::Dots, static_success_msg, Some(Color::Green));
                } else {
                    let status = response.status();
                    // Update spinner with error message
                    let error_msg = format!("Error sending to Teams ({}): {}", webhook_name, status);
                    let static_error_msg: &'static str = Box::leak(error_msg.into_boxed_str());
                    spinner.update(spinners::Dots, static_error_msg, Some(Color::Red));
                    failure_count += 1;
                }
            }
            Err(err) => {
                let err_msg = err.to_string();
                // Update spinner with error message
                let error_msg = format!("Error sending to Teams ({}): {}", webhook_name, err_msg);
                let static_error_msg: &'static str = Box::leak(error_msg.into_boxed_str());
                spinner.update(spinners::Dots, static_error_msg, Some(Color::Red));
                failure_count += 1;
            }
        }
    }
    
    // Update the spinner with the final result
    if !SPINNER_STOPPED.load(Ordering::SeqCst) {
        if failure_count == 0 && success_count > 0 {
            let message = format!("{} (Sent to {} webhooks)", success_message, success_count);
            let static_message: &'static str = Box::leak(message.into_boxed_str());
            spinner.success(static_message);
        } else if failure_count > 0 && success_count > 0 {
            let message = format!("Sent to {} Teams webhooks, failed to send to {} webhooks", success_count, failure_count);
            let static_message: &'static str = Box::leak(message.into_boxed_str());
            spinner.stop_and_persist("⚠️", static_message);
        } else {
            spinner.stop_and_persist("❌", "Failed to send summary to any Teams webhooks!");
        }
        SPINNER_STOPPED.store(true, Ordering::SeqCst);
    }
    
    Ok(())
}
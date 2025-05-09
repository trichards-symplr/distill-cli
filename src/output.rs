use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use config::Config;
use docx_rs::{Docx, Paragraph, Run};
use reqwest::Client as ReqwestClient;
use serde_json::json;
use spinoff::Spinner;

/// Writes summary content to a text file
pub fn write_text_file(summary_file_name: &str, summarized_text: &str, spinner: &mut Spinner) -> Result<()> {
    let ext = ".txt";
    let outfile = summary_file_name.to_owned() + ext;
    let output_file_path = Path::new(&outfile);
    let mut file = File::create(output_file_path)
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    file.write_all(summarized_text.as_bytes())
        .map_err(|e| anyhow::anyhow!("Error creating file: {}", e))?;

    // Simply update the spinner with success message
    spinner.success("Done!");
    
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Writes summary content to a Word document
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

    spinner.success("Done!");
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Writes summary content to a Markdown file
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

    spinner.success("Done!");
    println!("💾 Summary written to {}", output_file_path.display());
    
    Ok(())
}

/// Send a notification to Slack
pub async fn send_slack_notification(
    settings: &Config,
    spinner: &mut Spinner,
    summarized_text: &str,
) -> Result<()> {
    let client = ReqwestClient::new();

    let slack_webhook_endpoint = settings
        .get_string("slack.webhook_endpoint")
        .unwrap_or_default();

    if slack_webhook_endpoint.is_empty() {
        spinner.stop_and_persist(
            "⚠️",
            "Slack webhook endpoint is not configured. Skipping Slack notification.",
        );
        println!("Summary:\n{}\n", summarized_text);
        return Ok(());
    }

    let content = format!("A summarization job just completed:\n\n{}", summarized_text);
    let payload = json!({
        "content": content
    });
    
    let result = client
        .post(slack_webhook_endpoint)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;
        
    match result {
        Ok(response) => {
            if response.status().is_success() {
                spinner.success("Summary sent to Slack!");
            } else {
                let status = response.status();
                eprintln!("Error sending summary to Slack: {}", status);
                spinner.stop_and_persist("❌", "Failed to send summary to Slack!");
            }
        }
        Err(err) => {
            let err_msg = err.to_string();
            eprintln!("Error sending summary to Slack: {}", err_msg);
            spinner.stop_and_persist("❌", "Failed to send summary to Slack!");
        }
    }
    
    Ok(())
}

/// Send a notification to Microsoft Teams
pub async fn send_teams_notification(
    settings: &Config,
    spinner: &mut Spinner,
    summarized_text: &str,
    user_input: &str,
    success_message: &str,
) -> Result<()> {
    let client = ReqwestClient::new();

    let current_date = chrono::Local::now();
    let formatted_date = current_date.format("%m-%d-%Y %I:%M:%S %p").to_string();
    let tz = tz::TimeZone::local().expect("Unable to determine timezone");
    let tz_name = tz.find_current_local_time_type().expect("Could not find local timezone type").time_zone_designation();
    let date_header = format!("Date: {} {}", formatted_date, tz_name);

    let teams_webhook_endpoint = settings
        .get_string("teams.webhook_endpoint")
        .unwrap_or_default();

    if teams_webhook_endpoint.is_empty() {
        spinner.stop_and_persist(
            "⚠️",
            "Teams webhook endpoint is not configured. Skipping Teams notification.",
        );
        println!("Summary:\n{}\n", summarized_text);
        return Ok(());
    }

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

    let result = client
        .post(teams_webhook_endpoint)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;
        
    match result {
        Ok(response) => {
            if response.status().is_success() {
                spinner.success(success_message);
            } else {
                let status = response.status();
                eprintln!("Error sending summary to Teams: {}", status);
                spinner.stop_and_persist("❌", "Failed to send summary to Teams!");
            }
        }
        Err(err) => {
            let err_msg = err.to_string();
            eprintln!("Error sending summary to Teams: {}", err_msg);
            spinner.stop_and_persist("❌", "Failed to send summary to Teams!");
        }
    }

    Ok(())
}
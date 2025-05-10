//! # Summarize Module
//!
//! This module handles the text summarization functionality using Amazon Bedrock:
//! - Connects to the Amazon Bedrock service
//! - Formats the transcription text for the AI model
//! - Sends the text to the model for summarization
//! - Processes and returns the summarized text
//!
//! The module abstracts away the details of working with the AI model and provides
//! a simple interface for generating summaries from transcription text.
//!
//! ## Configuration
//! The module uses settings from config.toml to configure:
//! - The prompt template for summarization
//! - The AI model to use (default: Claude)
//! - Model parameters like max_tokens, temperature, etc.
//!
//! ## Usage
//! This module is typically used after transcription to condense long transcripts
//! into concise, readable summaries.

use aws_config::SdkConfig;
use aws_sdk_bedrockruntime::{primitives::Blob, Client};

use anyhow::{anyhow, Error};

use config::{Config, File};
use serde_json::json;
use spinoff::Spinner;
use std::str::from_utf8;

/// Summarizes transcribed text using Amazon Bedrock's AI models
///
/// # Arguments
///
/// * `config` - AWS SDK configuration
/// * `transcribed_text` - The text to summarize, typically from a transcription
/// * `spinner` - Progress spinner to update during the summarization process
///
/// # Returns
///
/// A Result containing the summarized text or an error
///
/// Loads the prompt template and model settings from config.toml,
/// formats the prompt with the transcribed text, and sends it to
/// the Amazon Bedrock model (default: Claude).
pub async fn summarize_text(
    config: &SdkConfig,
    transcribed_text: &str,
    spinner: &mut Spinner,
) -> Result<String, Error> {
    let client = Client::new(config);
    let settings = Config::builder()
        .add_source(File::with_name("config.toml"))
        .build()?;

    let prompt_template = settings.get_string("prompt.template").unwrap_or_default();

    let prompt = format!("{prompt_template}\n\n{transcribed_text}");

    // We're using the Anthropic Claude Messages API by default.
    // If you switch models, you may need to update `messages`
    // and/or `body`.
    // https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters.html
    // Claude: https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-anthropic-claude-messages.html
    let messages = json!([
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": prompt,
                }
            ]
        }
    ]);

    let body = json!(
        {
            "anthropic_version": settings.get_string("anthropic.anthropic_version").unwrap_or_default(),
            "max_tokens": settings.get_int("model.max_tokens").unwrap_or_default(),
            "system": settings.get_string("anthropic.system").unwrap_or_default(),
            "messages": messages,
            "temperature": settings.get_int("model.temperature").unwrap_or_default(),
            "top_p": settings.get_int("model.top_p").unwrap_or_default(),
            "top_k": settings.get_int("model.top_k").unwrap_or_default(),
        }
    )
    .to_string();

    let blob_body = Blob::new(body);

    spinner.update_text("Summarizing transcription...");
    let response = client
        .invoke_model()
        .body(blob_body)
        .content_type("application/json")
        .accept("application/json")
        .model_id(settings.get_string("model.model_id").unwrap_or_default())
        .send()
        .await;

    match response {
        Ok(output) => {
            let response_body = from_utf8(output.body.as_ref()).unwrap_or("");
            let response_json: serde_json::Value = serde_json::from_str(response_body).unwrap();

            let summarization = response_json["content"][0]["text"]
                .as_str()
                .unwrap()
                .replace("\\n", "\n");
            Ok(summarization.to_string())
        }
        Err(e) => Err(anyhow!(e)),
    }
}
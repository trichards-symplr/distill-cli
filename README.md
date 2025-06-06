# Distill CLI

Distill CLI is a tool for transcribing and summarizing audio files using AWS services. It leverages Amazon Transcribe for speech-to-text conversion and Amazon Bedrock's AI models for summarization.

## Table of Contents

- [Supported AWS Regions](#supported-aws-regions)
- [Install the Distill CLI](#install-the-distill-cli)
  - [Prerequisites](#prerequisites)
  - [Step 1: Clone the repo](#step-1-clone-the-repo)
  - [Step 2: Build from source](#step-2-build-from-source)
- [Usage](#usage)
  - [Application Flow](#application-flow)
- [Command Line Options](#command-line-options)
  - [Output Types Explained](#output-types-explained)
  - [Teams and Slack Integration](#teams-and-slack-integration)
- [Config settings](#config-settings)
  - [How to adjust model values](#how-to-adjust-model-values)
  - [Supported Bedrock models](#supported-bedrock-models)
  - [Configuring Webhook Endpoints](#configuring-webhook-endpoints)
  - [Teams Card Customization](#teams-card-customization)
- [Automation](#automation)
- [Security](#security)
- [License](#license)
- [Diagrams](#diagrams)
  - [Application Architecture](#application-architecture)
  - [Spinner Management Flow](#spinner-management-flow)

## Supported AWS Regions

Make sure your default region in your AWS config is on the list of [supported Bedrock regions](https://docs.aws.amazon.com/bedrock/latest/userguide/bedrock-regions.html). 

**Note**: If no region is set in your AWS CLI config, the Distill CLI will default to `us-east-1`.

To check your defaults, run:

```bash
aws configure list
```

# Install the Distill CLI

This project is written in Rust, and uses the AWS SDK for Rust to manage credentials and access AWS services, including S3, Transcribe and Bedrock. 

**IMPORTANT**: By using the Distill CLI, you may incur charges to your AWS account. 

## Prerequisites 

Before using the Distill CLI, you'll need: 

- [An AWS Account](https://portal.aws.amazon.com/gp/aws/developer/registration/index.html) configured with an [IAM user that has permissions](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html#Using_CreateAccessKey) to Amazon Transcribe, Amazon Bedrock, and Amazon S3. 
- [Configure the AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html) to access your AWS account.
- An S3 bucket to store audio files, or [create a new one](https://docs.aws.amazon.com/AmazonS3/latest/userguide/creating-bucket.html). 
- [Access to Anthropic's Claude 3](https://console.aws.amazon.com/bedrock/home?#/models) via the AWS Bedrock Console.
- [Rust and Cargo](https://www.rust-lang.org/tools/install) installed.

## Step 1: Clone the repo 

```bash
git clone https://github.com/awslabs/distill-cli.git && cd distill-cli
```

## Step 2: Build from source

Run the following command to build the Distill CLI from source. This will compile the code and create an optimized binary in `target/release`.

```bash
$ cargo build --release
```

You should see a message like this when the build is complete:

```bash
Compiling distill-cli v0.1.0 (/Projects/distill-cli)
    Finished release [optimized] target(s) in 18.07s
```

# Usage

Once installed, it's easy to use the Distill CLI. Each operation starts with:

```bash
./target/release/distill-cli [arguments]
```

Here's a simple example. By default, the Distill CLI will print the summary to terminal unless otherwise specified:

```bash
./target/release/distill-cli -i meeting.m4a
```

You'll see something similar to the following:

```bash
$ ./target/release/distill-cli -i meeting.m4a

🧙 Welcome to Distill CLI
✔ Choose a destination S3 bucket for your audio file · mys3bucket
⠐ Uploading file to S3...
⠐ Using bucket region eu-west-2...
⠒ Submitting transcription job
⠤ Waiting for transcription to complete...
⠤ Waiting for transcription to complete...
✓ Done!

Summary:
Here is a summary of the conversation:

The speakers discussed the recent Premier League matches involving Arsenal, Manchester City, and Liverpool. Arsenal beat Luton Town in their match, while Manchester City also won their game 4-1. This leaves Arsenal tied on points with Manchester City, but with a better goal differential, putting them temporarily in first place ahead of City. However, the speakers expect Liverpool, who are currently one or two points behind Arsenal, to regain the lead after their upcoming match against an opponent perceived as weak.

Key action items and follow-ups:

- Monitor Liverpool's next match results, as they are expected to go back into first place in the Premier League standings
- Keep track of the evolving points totals and goal differentials for Arsenal, Manchester City, and Liverpool as the title race continues
...
```

## Application Flow

When you run Distill CLI, it follows this process:

1. **Parse Arguments**: Processes your command-line options
2. **Load Configuration**: Reads settings from `config.toml`
3. **Select S3 Bucket**: Uses the bucket from config or prompts you to choose one
4. **Select Webhooks**: If using Teams or Slack output, prompts for webhook selection
5. **Upload Audio**: Sends your audio file to the selected S3 bucket with server-side encryption (AES-256)
6. **Transcribe Audio**: Uses Amazon Transcribe to convert speech to text
7. **Summarize Text**: Uses Amazon Bedrock to create a concise summary
8. **Process Output**: Delivers the summary in your chosen format
9. **Cleanup**: Optionally deletes the S3 object based on `--delete-s3-object`
10. **Save Transcript** (Optional): Saves the full transcript if `--save-transcript` is specified

# Command Line Options 

| Option | Required | Description |
| - | - | - |
| `-i`, `--input-audio-file` | Yes | Specify the audio file to be summarized. | 
| `-o`, `--output-type` | No | Specify the output format of the summary. Default is `terminal`.<br> **Accepted values**: `terminal`, `text`, `word`, `markdown`, `slack`, `slacksplit`, `teams`, `teamssplit` |
| `-s`, `--summary-file-name` | No | Base name for output files (without extension). Default is `summarized_output`. |
| `-l`, `--language-code` | No | Input language code. Default is `en-US`.<br> **Accepted values**: Check: [Amazon Transcribe Supported Languages Documentation](https://docs.aws.amazon.com/transcribe/latest/dg/supported-languages.html) | 
| `-d`, `--delete-s3-object` | No | Whether to delete the S3 object after processing. Default is `Y`. Set to `N` to keep files in S3. |
| `-t`, `--save-transcript` | No | Save the full transcript to a `.trans` file alongside the summary. |
| `-h`, `--help` | No | Provides help for the Distill CLI. |

## Output Types Explained

- **Terminal**: Displays the summary in the console (default)
- **Text**: Writes the summary to a `.txt` file
- **Word**: Creates a Microsoft Word (`.docx`) document with the summary
- **Markdown**: Creates a `.md` file with formatted summary
- **Slack**: Sends the summary to one or more Slack webhooks
- **SlackSplit**: Writes the summary to a `.txt` file AND sends it to Slack
- **Teams**: Sends the summary as an adaptive card to one or more Microsoft Teams webhooks
- **TeamsSplit**: Writes the summary to a `.txt` file AND sends it to Teams

### Teams and Slack Integration

For the **Teams** and **TeamsSplit** output types, you will be asked for a short title that will be used when creating the AdaptiveCard. 

To use Microsoft Teams or Slack, you'll need to configure webhook endpoints in your `config.toml` file. See the [Configuring Webhook Endpoints](#configuring-webhook-endpoints) section for details.

# Config settings

`config.toml` is used to manage config settings for the Distill CLI and must be in the execution directory of `distill-cli`.  

## How to adjust model values

The CLI is intended as a proof-of-concept, and as such is designed to support Anthropic's Claude 3 foundation model. The model, along with values such as max tokens and temperature are specified in [`config.toml`](./config.toml).

```
[model]
model_id = "anthropic.claude-3-sonnet-20240229-v1:0"
max_tokens = 2000
temperature = 1.0
top_p = 0.999
top_k = 40
```

Note: For newer models, you will need to use a cross-region model_id, for example:

```
model_id = "us.anthropic.claude-3-5-sonnet-20241022-v2:0"
max_tokens = 4096
temperature = 0.1
top_p = 0.999
top_k = 40
```

**IMPORTANT**: If changing to a model not provided by Anthropic, code changes may be required to `messages` and `body` in [`summarizer.rs`](./src/summarize.rs), as the structure of the messages passed to Bedrock may change. Anthropic's models, for example, currently use the [Messages API](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-anthropic-claude-messages.html). 

## Supported Bedrock models

You can view a list of available models at [Amazon Bedrock base model IDs](https://docs.aws.amazon.com/bedrock/latest/userguide/model-ids.html), or via the command line:

```
$ aws bedrock list-foundation-models

{
    "modelSummaries": [
        {
            "modelArn": "arn:aws:bedrock:us-east-1::foundation-model/amazon.titan-tg1-large",
            "modelId": "amazon.titan-tg1-large",
            "modelName": "Titan Text Large",
            "providerName": "Amazon",
            "inputModalities": [
                "TEXT"
            ],
            "outputModalities": [
                "TEXT"
            ],
            "responseStreamingSupported": true,
            "customizationsSupported": [],
            "inferenceTypesSupported": [
                "ON_DEMAND"
            ]
        },
        {
            "modelArn": "arn:aws:bedrock:us-east-1::foundation-model/amazon.titan-image-generator-v1:0",
            "modelId": "amazon.titan-image-generator-v1:0",
            "modelName": "Titan Image Generator G1",
            "providerName": "Amazon",
            "inputModalities": [
                "TEXT",
                "IMAGE"
            ],
            "outputModalities": [
                "IMAGE"
            ],
            "customizationsSupported": [
                "FINE_TUNING"
            ],
            "inferenceTypesSupported": [
                "PROVISIONED"
            ]
        },
        ...
    ]
}
```

## Configuring Webhook Endpoints

Distill CLI supports sending summaries to both Slack and Microsoft Teams through webhooks. You can configure either a single webhook endpoint or multiple webhook endpoints for each service.

### Single Webhook Configuration (Legacy)

For a single webhook, use the following format in your `config.toml`:

```toml
[slack]
webhook_endpoint = "https://hooks.slack.com/workflows/XYZ/ABC/123"

[teams]
webhook_endpoint = "https://prod-33.westus.logic.azure.com:443/workflows/....."
```

### Multiple Webhook Configuration

For multiple webhooks, use the array format with named endpoints:

```toml
[slack]
webhooks = [
  { name = "General Channel", endpoint = "https://hooks.slack.com/workflows/XYZ/ABC/123" },
  { name = "Project Team", endpoint = "https://hooks.slack.com/workflows/XYZ/DEF/456" },
  { name = "Management", endpoint = "https://hooks.slack.com/workflows/XYZ/GHI/789" }
]

[teams]
webhooks = [
  { name = "General Channel", endpoint = "https://prod-33.westus.logic.azure.com:443/workflows/..." },
  { name = "Project Team", endpoint = "https://prod-33.westus.logic.azure.com:443/workflows/..." },
  { name = "Management", endpoint = "https://prod-33.westus.logic.azure.com:443/workflows/..." }
]
```

### Webhook Selection

When using the Slack or Teams output types with multiple webhooks configured:

1. If only one webhook is configured, it will be used automatically without prompting.
2. If multiple webhooks are configured, you'll be presented with a multi-select dialog to choose which webhooks to send the summary to:

```
? Select Teams channels to send the summary to: ›
❯ □ General Channel
  □ Project Team
  □ Management
```

Use the arrow keys to navigate, space to toggle selection, and enter to confirm. By default, no webhooks are selected, so you need to explicitly choose which ones to use.

3. The summary will be sent to all selected webhooks, with progress updates for each.

### Creating Webhooks

- **For Slack**: Create a [Slack webhook](https://api.slack.com/messaging/webhooks) by setting up an incoming webhook app in your Slack workspace.
- **For Teams**: Create a Teams webhook by setting up a workflow in Microsoft Teams that will post to a chat or channel.

### Teams Card Customization

You can customize the appearance of the Teams adaptive card by configuring the icon settings in the `config.toml` file:

```toml
[teams.icon]
name = "Flash"
size = "Large"
style = "Filled"
color = "Accent"
```

#### Icon Settings

- **name**: The name of the icon to display. 
  - Possible values: Any icon name from the [Microsoft Adaptive Cards Icon Set](https://fluent2.microsoft.design/iconography), such as "Flash", "Star", "Bookmark", "Calendar", "CheckMark", "Contact", "Document", "Download", etc.

- **size**: The size of the icon.
  - Possible values: "Small", "Medium", "Large", "ExtraLarge"

- **style**: The style of the icon.
  - Possible values: "Default", "Filled", "Outline"

- **color**: The color of the icon.
  - Possible values: "Default", "Accent", "Good", "Warning", "Attention", or any valid HTML color code

For more information about Adaptive Cards in Microsoft Teams, refer to the [Microsoft Adaptive Cards documentation](https://learn.microsoft.com/en-us/adaptive-cards/).

## Automation

Two scripts have been provided that can be found under the **src/scripts** folder.

- distill.sh: Requires the creation of a "in" and "out" folders in the directory where you want to run this cript from. Additionally you must copy the **config.toml** file into the same root directory.
    - This script will iterate through all of the files in the "in" folder and produce a text file of the same name with the summary in the "out" folder. 
    - This script requires you to designate the AWS Account profile by setting it in the **AWS_PROFILE** variable at the top of the script.
- translit-mv: Utility script that will inspect the audio files in the "in" folder and rename them ensuring that transcription and summarization process will not fail to read the files.

## Security

- All data uploaded to S3 is automatically encrypted using AES-256 server-side encryption
- Data in transit is protected using HTTPS connections provided by the AWS SDK
- Webhook URLs for Slack and Teams should be treated as sensitive information and not committed to version control
- For security issue notifications and reporting vulnerabilities, see [CONTRIBUTING](CONTRIBUTING.md#security-issue-notifications)

## License

This project is licensed under the Apache-2.0 License.

## Diagrams

The project includes several diagrams to help visualize the application architecture and flow. These diagrams are located in the `/diagrams` directory at the root of the repository.

### Application Architecture

The application architecture diagrams in the `/diagrams` directory include:
- `class_diagram.puml` - Shows the relationships between modules
- `main_sequence.puml` - Illustrates the main application flow
- `aws_utils_sequence_diagram.puml` - Shows AWS service interactions
- `transcrribe_sequence.puml` - Details the transcription process
- `summarize_sequence.puml` - Illustrates the summarization process
- `output_sequence_diagram.puml` - Shows the output handling process

These diagrams help developers understand how the different components interact and the overall flow of the application.

### Spinner Management Flow

The `spinner_management_flow.puml` file in the `/diagrams` directory illustrates how progress spinners are managed throughout the application:
- Initialization of the `SPINNER_STOPPED` flag
- Proper updating of spinners during processing
- Safe stopping of spinners to prevent thread panics
- Different output paths and their spinner handling

This diagram is particularly important for understanding how to properly manage spinner threads when extending the application with new features.

To view these diagrams, you can use any PlantUML viewer or the online PlantUML server at http://www.plantuml.com/plantuml/uml/. The `/diagrams` directory also contains a README.md with more information about each diagram.
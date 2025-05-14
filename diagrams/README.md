# Diagrams

This directory contains diagrams for the Distill CLI application. These diagrams help visualize the application's architecture, flow, and component interactions.

## Class Diagram

The `class_diagram.puml` file contains a PlantUML diagram showing the class structure of the Distill CLI application. It illustrates the relationships between the different modules and their key functions.

## Sequence Diagrams

### Main Sequence

The `main_sequence.puml` file shows the high-level sequence of operations in the main application flow, from command-line argument parsing to output generation.

### AWS Utils Sequence

The `aws_utils_sequence_diagram.puml` file illustrates the interactions between the application and AWS services, including S3 bucket operations and configuration loading.

### Transcribe Sequence

The `transcribe_sequence.puml` file shows the sequence of operations for audio transcription using Amazon Transcribe, including job creation, monitoring, and result processing.

### Summarize Sequence

The `summarize_sequence.puml` file illustrates the process of summarizing transcribed text using Amazon Bedrock, including prompt formatting and model invocation.

### Output Sequence

The `output_sequence_diagram.puml` file shows the sequence of operations for different output formats (text, Word, Markdown) and notification channels (Slack, Teams).

## Flow Diagrams

### Spinner Management Flow

The `spinner_management_flow.puml` file contains a PlantUML diagram that illustrates how spinner threads are managed in the application. This diagram shows:

1. The initialization of the `SPINNER_STOPPED` flag at application start
2. The main processing flow with spinner updates
3. Different output paths (terminal, file, webhook)
4. The checks for `SPINNER_STOPPED` before stopping the spinner
5. Setting `SPINNER_STOPPED` to true after stopping the spinner
6. Using `spinner.update()` instead of `stop_and_persist()` for progress updates

## Viewing the Diagrams

To view these diagrams:

1. Install PlantUML: https://plantuml.com/starting
2. Use a PlantUML plugin for your IDE or editor
3. Or use the online PlantUML server: http://www.plantuml.com/plantuml/uml/

## Key Concepts

### Spinner Management

- **SPINNER_STOPPED Flag**: A global AtomicBool that tracks whether the spinner has been stopped
- **Spinner Update**: Use `spinner.update()` for progress updates without stopping the spinner
- **Spinner Stop**: Only stop the spinner once by checking the flag before stopping
- **Thread Safety**: The AtomicBool ensures thread-safe access to the flag

### Implementation Notes

The spinner management pattern is implemented across multiple modules:

- `output.rs`: Defines the `SPINNER_STOPPED` flag and `reset_spinner_flag()` function
- `main.rs`: Calls `reset_spinner_flag()` at application start
- All modules that use spinners: Check the flag before stopping the spinner

### Module Interactions

The diagrams collectively illustrate how the different modules interact:

1. `main.rs` orchestrates the overall flow
2. `aws_utils.rs` handles AWS service interactions
3. `transcribe.rs` manages audio transcription
4. `summarize.rs` handles text summarization
5. `output.rs` manages file output and notifications

Each module has specific responsibilities but works together to provide a seamless user experience.
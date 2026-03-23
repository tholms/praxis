use anyhow::Result;
use clap::Subcommand;

use crate::client::CliClient;
use crate::output::OutputFormat;

#[derive(Subcommand)]
pub enum SdkCommand {
    /// Send a prompt to an SDK-remote node
    Prompt {
        /// Node ID (or prefix)
        node_id: String,
        /// Prompt text
        text: String,
    },
    /// Approve a pending tool request
    Approve {
        /// Node ID (or prefix)
        node_id: String,
        /// Request ID from the tool permission event
        request_id: String,
    },
    /// Deny a pending tool request
    Deny {
        /// Node ID (or prefix)
        node_id: String,
        /// Request ID from the tool permission event
        request_id: String,
    },
    /// Disconnect an SDK-remote node
    Disconnect {
        /// Node ID (or prefix)
        node_id: String,
    },
    /// Toggle auto-approve for tool requests
    SetAutoApprove {
        /// Node ID (or prefix)
        node_id: String,
        /// on or off
        mode: String,
    },
}

pub async fn execute(client: &mut CliClient, command: SdkCommand, _output: &OutputFormat) -> Result<()> {
    match command {
        SdkCommand::Prompt { node_id, text } => {
            let result = client.send_sdk_prompt_interactive(&node_id, &text).await?;
            if result.is_error {
                eprintln!("Error: {}", result.result);
            } else {
                println!("{}", result.result);
            }
        }
        SdkCommand::Approve { node_id, request_id } => {
            client.send_sdk_tool_response(&node_id, &request_id, true).await?;
            println!("Tool approved.");
        }
        SdkCommand::Deny { node_id, request_id } => {
            client.send_sdk_tool_response(&node_id, &request_id, false).await?;
            println!("Tool denied.");
        }
        SdkCommand::Disconnect { node_id } => {
            client.send_sdk_disconnect(&node_id).await?;
            println!("Disconnect sent.");
        }
        SdkCommand::SetAutoApprove { node_id, mode } => {
            let auto_approve = matches!(mode.to_lowercase().as_str(), "on" | "true" | "yes" | "1");
            client.send_sdk_set_auto_approve(&node_id, auto_approve).await?;
            println!(
                "Auto-approve {}.",
                if auto_approve { "enabled" } else { "disabled" }
            );
        }
    }
    Ok(())
}

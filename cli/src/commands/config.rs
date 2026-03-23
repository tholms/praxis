use anyhow::Result;
use clap::Subcommand;

use crate::client::CliClient;
use crate::output::OutputFormat;

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Value to set
        value: String,
    },
    /// List all configuration values
    List,
}

pub async fn execute(
    client: &mut CliClient,
    command: ConfigCommand,
    output: &OutputFormat,
) -> Result<()> {
    match command {
        ConfigCommand::Get { key } => {
            let values = client.get_config(vec![key.clone()]).await?;
            match output {
                OutputFormat::Json => {
                    let val = values.get(&key).cloned().unwrap_or_default();
                    println!(
                        "{}",
                        serde_json::json!({ "key": key, "value": val })
                    );
                }
                _ => {
                    if let Some(value) = values.get(&key) {
                        println!("{} = {}", key, value);
                    } else {
                        println!("{} (not set)", key);
                    }
                }
            }
        }
        ConfigCommand::Set { key, value } => {
            let mut values = std::collections::HashMap::new();
            values.insert(key.clone(), value.clone());
            client.set_config(values).await?;
            println!("{} = {}", key, value);
        }
        ConfigCommand::List => {
            let values = client.get_config(vec![]).await?;
            match output {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&values)?);
                }
                _ => {
                    if values.is_empty() {
                        println!("No configuration values set.");
                    } else {
                        let mut keys: Vec<&String> = values.keys().collect();
                        keys.sort();
                        for key in keys {
                            println!("{} = {}", key, values[key]);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

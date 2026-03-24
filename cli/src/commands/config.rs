use anyhow::Result;
use clap::Subcommand;
use std::collections::HashMap;

use crate::client::Client;
use crate::output::print_success;

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get a config value by key
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
    /// List all config keys and values
    List,
}

pub async fn execute(client: &Client, command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Get { key } => get_config(client, &key).await,
        ConfigCommand::Set { key, value } => set_config(client, &key, &value).await,
        ConfigCommand::List => list_config(client).await,
    }
}

async fn get_config(client: &Client, key: &str) -> Result<()> {
    let values = client.get_config(vec![key.to_string()]).await?;
    match values.get(key) {
        Some(v) => println!("{}", v),
        None => eprintln!("Key '{}' is not set", key),
    }
    Ok(())
}

async fn set_config(client: &Client, key: &str, value: &str) -> Result<()> {
    let mut values = HashMap::new();
    values.insert(key.to_string(), value.to_string());
    client.set_config(values).await?;
    print_success("Saved.");
    Ok(())
}

async fn list_config(client: &Client) -> Result<()> {
    let values = client.get_all_config().await?;
    let mut pairs: Vec<(&String, &String)> = values.iter().collect();
    pairs.sort_by_key(|(k, _)| k.as_str());
    let max_key_len = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in pairs {
        println!("{:<width$}    {}", k, v, width = max_key_len);
    }
    Ok(())
}

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use super::SdkCommand;

//
// Dispatch an operator command to the SdkSession identified by node_id.
// Returns Ok(true) if the session was found, Ok(false) if not.
//

pub async fn send_to_session(
    sessions: &Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
    node_id: &str,
    cmd: SdkCommand,
) -> anyhow::Result<bool> {
    let sessions = sessions.read().await;

    //
    // Try exact match first, then fall back to prefix match so callers
    // can use abbreviated node IDs (e.g. first 8 chars from `node list`).
    //

    let tx = sessions.get(node_id).or_else(|| {
        let mut matches: Vec<_> = sessions
            .iter()
            .filter(|(k, _)| k.starts_with(node_id))
            .collect();
        if matches.len() == 1 {
            Some(matches.remove(0).1)
        } else {
            None
        }
    });

    if let Some(tx) = tx {
        tx.send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("SDK session channel closed"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

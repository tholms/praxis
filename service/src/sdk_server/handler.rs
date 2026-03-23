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
    if let Some(tx) = sessions.get(node_id) {
        tx.send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("SDK session channel closed"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

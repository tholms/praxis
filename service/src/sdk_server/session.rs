use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::WebSocket;
use tokio::sync::{mpsc, RwLock};

use lapin::Channel;

use common::SdkNodeState;

use super::{SdkCommand, SdkServerConfig};

pub struct SdkSession;

impl SdkSession {
    pub async fn run(
        _socket: WebSocket,
        _peer: SocketAddr,
        _config: SdkServerConfig,
        _sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
        _sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>,
        _broadcast_channel: Channel,
    ) {
        common::log_warn!("SdkSession::run not yet implemented");
    }
}

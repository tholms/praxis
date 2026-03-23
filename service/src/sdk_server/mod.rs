pub mod handler;
pub mod session;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::sync::{mpsc, RwLock};

use lapin::Channel;

use common::SdkNodeState;

use self::session::SdkSession;

//
// Commands sent from the operator (via handler dispatch) to an SdkSession.
//

#[derive(Debug)]
pub enum SdkCommand {
    Prompt {
        text: String,
        transaction_id: String,
    },
    ToolResponse {
        request_id: String,
        allow: bool,
    },
    SetAutoApprove {
        auto_approve: bool,
    },
    Interrupt,
    Disconnect,
}

//
// Configuration snapshot passed to the manager on start.
//

#[derive(Debug, Clone)]
pub struct SdkServerConfig {
    pub port: u16,
    pub bind: String,
    pub auth_token: String,
    pub system_prompt: String,
    pub permission_mode: String,
    pub max_turns: u32,
    pub auto_approve: bool,
}

//
// Shared state for the Axum WebSocket handler.
//

struct SdkServerState {
    config: SdkServerConfig,
    sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
    sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>,
    broadcast_channel: Channel,
}

pub struct SdkServerManager {
    shutdown_tx: RwLock<Option<tokio::sync::oneshot::Sender<()>>>,
    sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
    sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>,
}

impl SdkServerManager {
    pub fn new() -> Self {
        Self {
            shutdown_tx: RwLock::new(None),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            sdk_nodes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_shared_nodes(sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>) -> Self {
        Self {
            shutdown_tx: RwLock::new(None),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            sdk_nodes,
        }
    }

    pub fn sdk_nodes(&self) -> &Arc<RwLock<Vec<SdkNodeState>>> {
        &self.sdk_nodes
    }

    pub fn sessions(&self) -> &Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>> {
        &self.sessions
    }

    pub async fn is_running(&self) -> bool {
        self.shutdown_tx.read().await.is_some()
    }

    pub async fn start(
        &self,
        config: SdkServerConfig,
        broadcast_channel: Channel,
    ) -> anyhow::Result<()> {
        self.stop().await;

        let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let state = Arc::new(SdkServerState {
            config,
            sessions: Arc::clone(&self.sessions),
            sdk_nodes: Arc::clone(&self.sdk_nodes),
            broadcast_channel,
        });

        let app = Router::new()
            .route("/", get(ws_upgrade_handler))
            .with_state(state);

        common::log_info!("SDK server starting on {}", addr);

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    common::log_error!("SDK server failed to bind {}: {}", addr, e);
                    return;
                }
            };

            let server = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            );

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        common::log_error!("SDK server error: {}", e);
                    }
                }
                _ = async {
                    let _ = shutdown_rx.await;
                } => {
                    common::log_info!("SDK server stopped");
                }
            }
        });

        *self.shutdown_tx.write().await = Some(shutdown_tx);
        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }

        //
        // Clear all sessions and nodes on stop.
        //

        self.sessions.write().await.clear();
        self.sdk_nodes.write().await.clear();
    }
}

impl Default for SdkServerManager {
    fn default() -> Self {
        Self::new()
    }
}

//
// Axum WebSocket upgrade handler.
//

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SdkServerState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    //
    // Validate auth token if configured.
    //

    if !state.config.auth_token.is_empty() {
        let authorized = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                v.strip_prefix("Bearer ")
                    .unwrap_or(v)
                    .trim()
                    == state.config.auth_token
            })
            .unwrap_or(false);

        if !authorized {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    common::log_info!("SDK connection from {}", peer);

    ws.on_upgrade(move |socket| {
        SdkSession::run(
            socket,
            peer,
            state.config.clone(),
            Arc::clone(&state.sessions),
            Arc::clone(&state.sdk_nodes),
            state.broadcast_channel.clone(),
        )
    })
}

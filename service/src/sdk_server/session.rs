use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use lapin::Channel;

use common::sdk_protocol::{
    self, ControlRequestMessage, SdkInboundMessage,
};
use common::{
    publish_json_exchange, ClientBroadcastMessage, SdkNodeState, CLIENT_BROADCAST_EXCHANGE,
};

use super::{SdkCommand, SdkServerConfig};

pub struct SdkSession;

impl SdkSession {
    pub async fn run(
        socket: WebSocket,
        peer: SocketAddr,
        config: SdkServerConfig,
        sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
        sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>,
        broadcast_channel: Channel,
    ) {
        let node_id = Uuid::new_v4().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SdkCommand>(64);

        let mut session = SessionInner {
            node_id: node_id.clone(),
            peer,
            config,
            socket,
            sessions,
            sdk_nodes,
            broadcast_channel,
            cmd_rx,
            session_id: String::new(),
            auto_approve: false,
            transaction_id: None,
        };

        session.auto_approve = session.config.auto_approve;

        //
        // Register the command sender so operators can reach this session.
        //

        session
            .sessions
            .write()
            .await
            .insert(node_id.clone(), cmd_tx);

        if let Err(e) = session.run_inner().await {
            common::log_error!("SDK session {} error: {}", node_id, e);
        }

        //
        // Cleanup: deregister session and node, broadcast disconnect.
        //

        session.sessions.write().await.remove(&node_id);
        session
            .sdk_nodes
            .write()
            .await
            .retain(|n| n.node_id != node_id);

        let _ = publish_json_exchange(
            &session.broadcast_channel,
            CLIENT_BROADCAST_EXCHANGE,
            &ClientBroadcastMessage::SdkNodeDisconnected {
                node_id: node_id.clone(),
            },
        )
        .await;

        common::log_info!("SDK session {} disconnected", node_id);
    }
}

struct SessionInner {
    node_id: String,
    peer: SocketAddr,
    config: SdkServerConfig,
    socket: WebSocket,
    sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SdkCommand>>>>,
    sdk_nodes: Arc<RwLock<Vec<SdkNodeState>>>,
    broadcast_channel: Channel,
    cmd_rx: mpsc::Receiver<SdkCommand>,
    session_id: String,
    auto_approve: bool,
    transaction_id: Option<String>,
}

impl SessionInner {
    async fn run_inner(&mut self) -> anyhow::Result<()> {
        self.handshake().await?;
        self.main_loop().await
    }

    //
    // Handshake: send initialize immediately, then collect system/init (optional)
    // and control_response in any order. Handshake completes on control_response.
    //

    async fn handshake(&mut self) -> anyhow::Result<()> {
        let init_msg = sdk_protocol::make_initialize_request(
            &self.config.system_prompt,
            &self.config.permission_mode,
            self.config.max_turns,
        );
        self.send(&init_msg).await?;

        let mut got_system_init = false;
        let mut got_control_response = false;
        let mut node_info = SdkNodeState {
            node_id: self.node_id.clone(),
            cwd: String::new(),
            model: String::new(),
            tools: Vec::new(),
            claude_code_version: String::new(),
            permission_mode: self.config.permission_mode.clone(),
            connected_at: chrono::Utc::now(),
            auto_approve: self.auto_approve,
            peer_address: self.peer.to_string(),
        };

        while !got_control_response {
            let msg = self.recv().await?;
            match msg {
                SdkInboundMessage::System(sys) if !got_system_init => {
                    self.session_id = sys.session_id.clone();
                    node_info.cwd = sys.cwd.clone();
                    node_info.model = sys.model.clone();
                    node_info.tools = sys.tools.clone();
                    node_info.claude_code_version = sys.claude_code_version.clone();
                    if !sys.permission_mode.is_empty() {
                        node_info.permission_mode = sys.permission_mode.clone();
                    }
                    got_system_init = true;
                }
                SdkInboundMessage::ControlResponse(cr) if !got_control_response => {
                    got_control_response = true;
                    if !got_system_init {
                        //
                        // Seed node info from control_response if system/init
                        // never arrived. The response payload carries model,
                        // commands (tools), pid, permissionMode.
                        //

                        if let Some(resp) = cr.response.as_object() {
                            if let Some(inner) = resp.get("response").and_then(|r| r.as_object()) {
                                if let Some(m) = inner.get("model").and_then(|v| v.as_str()) {
                                    node_info.model = m.to_string();
                                }
                                if let Some(pm) = inner.get("permissionMode").and_then(|v| v.as_str()) {
                                    node_info.permission_mode = pm.to_string();
                                }
                            }
                        }
                    }
                }
                _ => {} // Skip hook_started, hook_response, keep_alive during handshake
            }
        }

        //
        // Register virtual node and broadcast connection.
        //

        self.sdk_nodes.write().await.push(node_info.clone());

        let _ = publish_json_exchange(
            &self.broadcast_channel,
            CLIENT_BROADCAST_EXCHANGE,
            &ClientBroadcastMessage::SdkNodeConnected {
                node_id: self.node_id.clone(),
                info: node_info,
            },
        )
        .await;

        common::log_info!(
            "SDK session {} handshake complete (session_id={})",
            self.node_id,
            self.session_id
        );

        Ok(())
    }

    //
    // Main loop: multiplex WebSocket inbound and operator commands.
    //

    async fn main_loop(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                ws_msg = self.socket.next() => {
                    match ws_msg {
                        Some(Ok(Message::Text(text))) => {
                            let msgs = sdk_protocol::decode_frame(&text);
                            for msg in msgs {
                                self.handle_inbound(msg).await?;
                            }
                        }
                        Some(Ok(Message::Binary(data))) => {
                            if let Ok(text) = String::from_utf8(data.to_vec()) {
                                let msgs = sdk_protocol::decode_frame(&text);
                                for msg in msgs {
                                    self.handle_inbound(msg).await?;
                                }
                            }
                        }
                        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(_)) => break,
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd).await?,
                        None => break,
                    }
                }
            }
        }
        Ok(())
    }

    fn short_id(&self) -> &str {
        self.node_id.get(..8).unwrap_or(&self.node_id)
    }

    async fn handle_inbound(&mut self, msg: SdkInboundMessage) -> anyhow::Result<()> {
        match msg {
            SdkInboundMessage::Assistant(assistant) => {
                common::log_debug!("[sdk:{}] assistant message received", self.short_id());
                let _ = publish_json_exchange(
                    &self.broadcast_channel,
                    CLIENT_BROADCAST_EXCHANGE,
                    &ClientBroadcastMessage::SdkAssistantMessage {
                        node_id: self.node_id.clone(),
                        content: assistant.message.clone(),
                        session_id: assistant.session_id.clone(),
                    },
                )
                .await;
            }
            SdkInboundMessage::Result(result) => {
                common::log_info!(
                    "[sdk:{}] result received (error={}, turns={}, stop={})",
                    self.short_id(), result.is_error, result.num_turns, result.stop_reason
                );
                let _ = publish_json_exchange(
                    &self.broadcast_channel,
                    CLIENT_BROADCAST_EXCHANGE,
                    &ClientBroadcastMessage::SdkResult {
                        node_id: self.node_id.clone(),
                        transaction_id: self.transaction_id.take().unwrap_or_default(),
                        result: result.result.clone(),
                        is_error: result.is_error,
                        duration_ms: result.duration_ms,
                        num_turns: result.num_turns,
                        stop_reason: result.stop_reason.clone(),
                    },
                )
                .await;
            }
            SdkInboundMessage::ControlRequest(cr) => {
                let subtype = cr.request.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                common::log_debug!("[sdk:{}] control_request subtype={}", self.short_id(), subtype);
                if subtype == "can_use_tool" {
                    self.handle_tool_request(&cr).await?;
                }
            }
            SdkInboundMessage::KeepAlive { .. } => {
                let ka = sdk_protocol::make_keep_alive();
                self.send(&ka).await?;
            }
            other => {
                common::log_debug!("[sdk:{}] unhandled inbound: {:?}", self.short_id(), other);
            }
        }
        Ok(())
    }

    async fn handle_tool_request(&mut self, cr: &ControlRequestMessage) -> anyhow::Result<()> {
        let tool_name = cr.request.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let input = cr.request.get("input").cloned().unwrap_or(serde_json::Value::Object(Default::default()));
        let description = cr.request.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if self.auto_approve {
            common::log_debug!("[sdk:{}] auto-approving tool: {}", self.short_id(), tool_name);
            let resp = sdk_protocol::make_control_response_allow(&cr.request_id, &input);
            self.send(&resp).await?;
        } else {
            common::log_info!(
                "[sdk:{}] tool permission request: {} (request_id={})",
                self.short_id(), tool_name, cr.request_id
            );

            //
            // Park the request and surface to operator.
            //

            let _ = publish_json_exchange(
                &self.broadcast_channel,
                CLIENT_BROADCAST_EXCHANGE,
                &ClientBroadcastMessage::SdkToolPermissionRequest {
                    node_id: self.node_id.clone(),
                    request_id: cr.request_id.clone(),
                    tool_name,
                    input,
                    description,
                },
            )
            .await;
        }
        Ok(())
    }

    async fn handle_command(&mut self, cmd: SdkCommand) -> anyhow::Result<()> {
        match cmd {
            SdkCommand::Prompt { text, transaction_id } => {
                common::log_info!(
                    "[sdk:{}] sending prompt (tx={}, len={})",
                    self.short_id(), &transaction_id[..8], text.len()
                );
                self.transaction_id = Some(transaction_id);
                let msg = sdk_protocol::make_user_message(&text, &self.session_id);
                self.send(&msg).await?;
            }
            SdkCommand::ToolResponse { request_id, allow } => {
                common::log_info!(
                    "[sdk:{}] tool response: allow={} (request_id={})",
                    self.short_id(), allow, request_id
                );
                //
                // For interactive approval, we need the original tool_input to echo
                // back. For MVP, send empty object -- this works for most tools.
                //

                let resp = if allow {
                    sdk_protocol::make_control_response_allow(
                        &request_id,
                        &serde_json::json!({}),
                    )
                } else {
                    sdk_protocol::make_control_response_deny(&request_id, "Operator denied")
                };
                self.send(&resp).await?;
            }
            SdkCommand::SetAutoApprove { auto_approve } => {
                common::log_info!("[sdk:{}] auto_approve set to {}", self.short_id(), auto_approve);
                self.auto_approve = auto_approve;

                //
                // Update the node registry so the UI reflects the change.
                //

                let mut nodes = self.sdk_nodes.write().await;
                if let Some(node) = nodes.iter_mut().find(|n| n.node_id == self.node_id) {
                    node.auto_approve = auto_approve;
                }
            }
            SdkCommand::Interrupt => {
                common::log_info!("[sdk:{}] sending interrupt", self.short_id());
                let msg = sdk_protocol::make_interrupt();
                self.send(&msg).await?;
            }
            SdkCommand::Disconnect => {
                common::log_info!("[sdk:{}] disconnecting (operator request)", self.short_id());
                let msg = sdk_protocol::make_end_session("operator_disconnect");
                self.send(&msg).await?;
                let _ = self.socket.close().await;
            }
        }
        Ok(())
    }

    //
    // Low-level WebSocket I/O.
    //

    async fn send(&mut self, msg: &serde_json::Value) -> anyhow::Result<()> {
        let data = sdk_protocol::encode(msg);
        self.socket
            .send(Message::Text(data.into()))
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket send error: {}", e))
    }

    async fn recv(&mut self) -> anyhow::Result<SdkInboundMessage> {
        loop {
            let msgs = self.recv_raw().await?;
            if let Some(msg) = msgs.into_iter().next() {
                return Ok(msg);
            }
        }
    }

    async fn recv_raw(&mut self) -> anyhow::Result<Vec<SdkInboundMessage>> {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let msgs = sdk_protocol::decode_frame(&text);
                    if !msgs.is_empty() {
                        return Ok(msgs);
                    }
                }
                Some(Ok(Message::Binary(data))) => {
                    if let Ok(text) = String::from_utf8(data.to_vec()) {
                        let msgs = sdk_protocol::decode_frame(&text);
                        if !msgs.is_empty() {
                            return Ok(msgs);
                        }
                    }
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                Some(Ok(Message::Close(_))) | None => {
                    return Err(anyhow::anyhow!("WebSocket closed"));
                }
                Some(Err(e)) => {
                    return Err(anyhow::anyhow!("WebSocket error: {}", e));
                }
            }
        }
    }
}

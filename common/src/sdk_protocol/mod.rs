use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

//
// Inbound messages (received from Claude Code).
// Deserialized from NDJSON lines on the WebSocket.
//

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum SdkInboundMessage {
    #[serde(rename = "system")]
    System(SystemMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "control_request")]
    ControlRequest(ControlRequestMessage),
    #[serde(rename = "control_response")]
    ControlResponse(ControlResponseMessage),
    #[serde(rename = "result")]
    Result(ResultMessage),
    #[serde(rename = "keep_alive")]
    KeepAlive {},
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemMessage {
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<Value>,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: String,
    #[serde(default)]
    pub slash_commands: Vec<Value>,
    #[serde(default, rename = "apiKeySource")]
    pub api_key_source: String,
    #[serde(default)]
    pub betas: Vec<String>,
    #[serde(default)]
    pub claude_code_version: String,
    #[serde(default)]
    pub uuid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    #[serde(default)]
    pub message: Value,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub uuid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlRequestMessage {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub request: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlResponseMessage {
    #[serde(default)]
    pub response: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultMessage {
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub stop_reason: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub duration_api_ms: u64,
    #[serde(default)]
    pub num_turns: u32,
}

//
// Outbound message constructors (sent to Claude Code).
//

pub fn make_initialize_request(
    system_prompt: &str,
    permission_mode: &str,
    max_turns: u32,
) -> Value {
    serde_json::json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": {
            "subtype": "initialize",
            "systemPrompt": system_prompt,
            "appendSystemPrompt": "",
            "permissionMode": permission_mode,
            "maxTurns": max_turns,
            "maxBudgetUsd": null,
            "hooks": {},
            "sdkMcpServers": [],
            "jsonSchema": null,
            "promptSuggestions": [],
            "agentProgressSummaries": []
        }
    })
}

pub fn make_user_message(content: &str, session_id: &str) -> Value {
    serde_json::json!({
        "type": "user",
        "session_id": session_id,
        "message": {
            "role": "user",
            "content": content
        },
        "parent_tool_use_id": null,
        "uuid": Uuid::new_v4().to_string()
    })
}

//
// Tool approval response. CRITICAL: uses nested response.response structure.
// Claude.exe matches by response.request_id (not top-level).
// For allow: updatedInput is required (echo back original tool input).
// For deny: message is required (can be empty string).
//

pub fn make_control_response_allow(request_id: &str, tool_input: &Value) -> Value {
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": {
                "behavior": "allow",
                "updatedInput": tool_input
            }
        }
    })
}

pub fn make_control_response_deny(request_id: &str, message: &str) -> Value {
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": {
                "behavior": "deny",
                "message": message
            }
        }
    })
}

pub fn make_keep_alive() -> Value {
    serde_json::json!({"type": "keep_alive"})
}

pub fn make_end_session(reason: &str) -> Value {
    serde_json::json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": {
            "subtype": "end_session",
            "reason": reason
        }
    })
}

pub fn make_interrupt() -> Value {
    serde_json::json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": {
            "subtype": "interrupt"
        }
    })
}

//
// Wire encoding: NDJSON over WebSocket text frames.
// Each message is JSON + "\n". A single frame may contain multiple messages.
//

pub fn encode(msg: &Value) -> String {
    let mut s = serde_json::to_string(msg).unwrap_or_default();
    s.push('\n');
    s
}

pub fn decode_frame(data: &str) -> Vec<SdkInboundMessage> {
    data.split('\n')
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_adds_newline() {
        let msg = make_keep_alive();
        let encoded = encode(&msg);
        assert!(encoded.ends_with('\n'));
        assert!(!encoded[..encoded.len() - 1].contains('\n'));
    }

    #[test]
    fn test_decode_frame_single_message() {
        let frame = r#"{"type":"keep_alive"}"#.to_string() + "\n";
        let msgs = decode_frame(&frame);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], SdkInboundMessage::KeepAlive { .. }));
    }

    #[test]
    fn test_decode_frame_multiple_messages() {
        let frame = format!(
            "{}\n{}\n",
            r#"{"type":"keep_alive"}"#,
            r#"{"type":"result","subtype":"success","session_id":"s1","uuid":"u1","result":"hello","stop_reason":"end_turn","is_error":false,"duration_ms":100,"duration_api_ms":50,"num_turns":1}"#
        );
        let msgs = decode_frame(&frame);
        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[0], SdkInboundMessage::KeepAlive { .. }));
        assert!(matches!(msgs[1], SdkInboundMessage::Result(_)));
    }

    #[test]
    fn test_decode_frame_skips_empty_lines() {
        let frame = "\n\n{\"type\":\"keep_alive\"}\n\n";
        let msgs = decode_frame(frame);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_decode_frame_skips_invalid_json() {
        let frame = "not-json\n{\"type\":\"keep_alive\"}\n";
        let msgs = decode_frame(frame);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_decode_system_init() {
        let frame = r#"{"type":"system","subtype":"init","session_id":"s1","cwd":"C:\\test","model":"claude-sonnet-4-5","tools":["Bash","Read"],"permissionMode":"default","claude_code_version":"1.0.0","uuid":"u1"}"#.to_string() + "\n";
        let msgs = decode_frame(&frame);
        assert_eq!(msgs.len(), 1);
        if let SdkInboundMessage::System(sys) = &msgs[0] {
            assert_eq!(sys.subtype, "init");
            assert_eq!(sys.session_id, "s1");
            assert_eq!(sys.model, "claude-sonnet-4-5");
            assert_eq!(sys.tools, vec!["Bash", "Read"]);
        } else {
            panic!("Expected System message");
        }
    }

    #[test]
    fn test_decode_assistant_message() {
        let frame = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]},"session_id":"s1","uuid":"u1"}"#.to_string() + "\n";
        let msgs = decode_frame(&frame);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], SdkInboundMessage::Assistant(_)));
    }

    #[test]
    fn test_decode_control_request() {
        let frame = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"}}}"#.to_string() + "\n";
        let msgs = decode_frame(&frame);
        assert_eq!(msgs.len(), 1);
        if let SdkInboundMessage::ControlRequest(cr) = &msgs[0] {
            assert_eq!(cr.request_id, "r1");
            assert_eq!(cr.request["subtype"], "can_use_tool");
            assert_eq!(cr.request["tool_name"], "Bash");
        } else {
            panic!("Expected ControlRequest");
        }
    }

    #[test]
    fn test_make_control_response_allow_structure() {
        let input = serde_json::json!({"command": "ls -la"});
        let msg = make_control_response_allow("req-123", &input);
        assert_eq!(msg["type"], "control_response");
        assert_eq!(msg["response"]["subtype"], "success");
        assert_eq!(msg["response"]["request_id"], "req-123");
        assert_eq!(msg["response"]["response"]["behavior"], "allow");
        assert_eq!(msg["response"]["response"]["updatedInput"]["command"], "ls -la");
    }

    #[test]
    fn test_make_control_response_deny_structure() {
        let msg = make_control_response_deny("req-456", "Not allowed");
        assert_eq!(msg["type"], "control_response");
        assert_eq!(msg["response"]["request_id"], "req-456");
        assert_eq!(msg["response"]["response"]["behavior"], "deny");
        assert_eq!(msg["response"]["response"]["message"], "Not allowed");
    }

    #[test]
    fn test_make_initialize_request_fields() {
        let msg = make_initialize_request("You are helpful.", "bypassPermissions", 20);
        assert_eq!(msg["type"], "control_request");
        assert_eq!(msg["request"]["subtype"], "initialize");
        assert_eq!(msg["request"]["systemPrompt"], "You are helpful.");
        assert_eq!(msg["request"]["permissionMode"], "bypassPermissions");
        assert_eq!(msg["request"]["maxTurns"], 20);
    }

    #[test]
    fn test_make_user_message_fields() {
        let msg = make_user_message("Hello!", "session-1");
        assert_eq!(msg["type"], "user");
        assert_eq!(msg["session_id"], "session-1");
        assert_eq!(msg["message"]["role"], "user");
        assert_eq!(msg["message"]["content"], "Hello!");
        assert!(msg["uuid"].as_str().is_some());
    }

    #[test]
    fn test_make_end_session_fields() {
        let msg = make_end_session("done");
        assert_eq!(msg["type"], "control_request");
        assert_eq!(msg["request"]["subtype"], "end_session");
        assert_eq!(msg["request"]["reason"], "done");
    }
}

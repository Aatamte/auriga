use crate::jsonrpc::{Request, Response};
use crate::{McpEvent, McpRequest, McpResponse};
use serde_json::json;
use std::sync::mpsc;

const TOOL_LIST_AGENTS: &str = "list_agents";
const TOOL_SEND_MESSAGE: &str = "send_message";
const TOOL_LIST_CONTEXT: &str = "list_context";
const TOOL_GET_CONTEXT: &str = "get_context";

/// Handle an MCP JSON-RPC request and return a response.
/// For tool calls, sends an McpEvent to the main loop and blocks for the response.
pub fn handle_request(req: &Request, event_tx: &mpsc::Sender<McpEvent>) -> Option<Response> {
    match req.method.as_str() {
        "initialize" => Some(handle_initialize(req)),
        "notifications/initialized" => None, // notification, no response
        "tools/list" => Some(handle_tools_list(req)),
        "tools/call" => Some(handle_tools_call(req, event_tx)),
        _ => Some(Response::error(
            req.id.clone(),
            -32601,
            format!("Method not found: {}", req.method),
        )),
    }
}

fn handle_initialize(req: &Request) -> Response {
    Response::success(
        req.id.clone(),
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "orchestrator",
                "version": "0.1.0"
            },
            "instructions": "You are one of several AI agents running inside an orchestrator TUI. The orchestrator manages multiple agents working in the same project. Your agent name is in the ORCHESTRATOR_AGENT_NAME environment variable — use it when sending messages so other agents know who you are. Use list_agents to discover other agents (it returns each agent's UUID, name, and status). Use send_message to communicate with them. Coordinate with other agents to avoid duplicate work and share findings. Use list_context to discover repository context documents that describe the codebase architecture, conventions, and patterns. Use get_context to read a specific document by name."
        }),
    )
}

fn handle_tools_list(req: &Request) -> Response {
    Response::success(
        req.id.clone(),
        json!({
            "tools": [
                {
                    "name": TOOL_LIST_AGENTS,
                    "description": "List all agents currently running in the orchestrator. Returns each agent's UUID, name, and current status (Idle or Working). Use this to discover which agents are available before sending messages.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": TOOL_SEND_MESSAGE,
                    "description": "Send a message to another agent. The message is delivered to the target agent's input with your name attached so they know who sent it. Returns immediately after delivery. Use list_agents first to get valid agent names.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "from_agent_name": {
                                "type": "string",
                                "description": "Your own agent name (from ORCHESTRATOR_AGENT_NAME env var). This is included in the message so the receiver knows who sent it."
                            },
                            "to_agent_name": {
                                "type": "string",
                                "description": "The exact name of the target agent (e.g. 'claude #a3f7b2c1'). Must match a name from list_agents."
                            },
                            "message": {
                                "type": "string",
                                "description": "The message to send. This will be delivered as input to the target agent."
                            }
                        },
                        "required": ["from_agent_name", "to_agent_name", "message"]
                    }
                },
                {
                    "name": TOOL_LIST_CONTEXT,
                    "description": "List repository context documents that describe this codebase — its architecture, design principles, coding conventions, and canonical examples. Call this when you need to understand the project structure, find the correct patterns to follow, or orient yourself in an unfamiliar area.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": TOOL_GET_CONTEXT,
                    "description": "Retrieve the full content of a repository context document by name. Use list_context first to see available documents and their descriptions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "The document name from list_context (e.g. 'map', 'principles', 'examples')."
                            }
                        },
                        "required": ["name"]
                    }
                }
            ]
        }),
    )
}

fn handle_tools_call(req: &Request, event_tx: &mpsc::Sender<McpEvent>) -> Response {
    let tool_name = req
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arguments = req.params.get("arguments").cloned().unwrap_or(json!({}));

    let mcp_request = match tool_name {
        TOOL_LIST_AGENTS => McpRequest::ListAgents,
        TOOL_SEND_MESSAGE => {
            let from_agent_name = match arguments.get("from_agent_name").and_then(|v| v.as_str()) {
                Some(name) => name.to_string(),
                None => {
                    return tool_error(req, "Missing required parameter: from_agent_name");
                }
            };
            let to_agent_name = match arguments.get("to_agent_name").and_then(|v| v.as_str()) {
                Some(name) => name.to_string(),
                None => {
                    return tool_error(req, "Missing required parameter: to_agent_name");
                }
            };
            let message = match arguments.get("message").and_then(|v| v.as_str()) {
                Some(msg) => msg.to_string(),
                None => {
                    return tool_error(req, "Missing required parameter: message");
                }
            };
            McpRequest::SendMessage {
                from_agent_name,
                to_agent_name,
                message,
            }
        }
        TOOL_LIST_CONTEXT => McpRequest::ListContext,
        TOOL_GET_CONTEXT => {
            let name = match arguments.get("name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => return tool_error(req, "Missing required parameter: name"),
            };
            McpRequest::GetContext { name }
        }
        _ => {
            return tool_error(req, &format!("Unknown tool: {}", tool_name));
        }
    };

    // Send to main loop and wait for response
    let (response_tx, response_rx) = mpsc::channel();
    let event = McpEvent {
        request: mcp_request,
        response_tx,
    };

    if event_tx.send(event).is_err() {
        return tool_error(req, "Orchestrator is shutting down");
    }

    match response_rx.recv() {
        Ok(McpResponse::Agents(agents)) => {
            let text = serde_json::to_string_pretty(&agents).unwrap_or_default();
            tool_success(req, &text)
        }
        Ok(McpResponse::MessageSent) => tool_success(req, "Message delivered to agent."),
        Ok(McpResponse::ContextList(docs)) => {
            let text = serde_json::to_string_pretty(&docs).unwrap_or_default();
            tool_success(req, &text)
        }
        Ok(McpResponse::ContextDoc(content)) => tool_success(req, &content),
        Ok(McpResponse::Error(msg)) => tool_error(req, &msg),
        Err(_) => tool_error(req, "Failed to get response from orchestrator"),
    }
}

fn tool_success(req: &Request, text: &str) -> Response {
    Response::success(
        req.id.clone(),
        json!({
            "content": [{"type": "text", "text": text}],
            "isError": false
        }),
    )
}

fn tool_error(req: &Request, message: &str) -> Response {
    Response::success(
        req.id.clone(),
        json!({
            "content": [{"type": "text", "text": message}],
            "isError": true
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str, params: serde_json::Value) -> Request {
        Request {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn initialize_returns_capabilities_and_instructions() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request("initialize", json!({}));
        let resp = handle_request(&req, &tx).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "orchestrator");
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["instructions"]
            .as_str()
            .unwrap()
            .contains("ORCHESTRATOR_AGENT_NAME"));
    }

    #[test]
    fn initialized_notification_returns_none() {
        let (tx, _rx) = mpsc::channel();
        let req = Request {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: json!(null),
        };
        assert!(handle_request(&req, &tx).is_none());
    }

    #[test]
    fn tools_list_returns_four_tools() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request("tools/list", json!({}));
        let resp = handle_request(&req, &tx).unwrap();
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"list_agents"));
        assert!(names.contains(&"send_message"));
        assert!(names.contains(&"list_context"));
        assert!(names.contains(&"get_context"));
    }

    #[test]
    fn send_message_requires_from_agent_name() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "send_message", "arguments": {"to_agent_name": "x", "message": "hi"}}),
        );
        let resp = handle_request(&req, &tx).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("from_agent_name"));
    }

    #[test]
    fn tools_call_unknown_tool_returns_error() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "nonexistent", "arguments": {}}),
        );
        let resp = handle_request(&req, &tx).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
    }

    #[test]
    fn tools_call_list_agents_sends_event() {
        let (tx, rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "list_agents", "arguments": {}}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &tx));

        let event = rx.recv().unwrap();
        assert!(matches!(event.request, McpRequest::ListAgents));
        event
            .response_tx
            .send(McpResponse::Agents(vec![crate::AgentInfo {
                id: "abc-123".to_string(),
                name: "claude #a3f7b2c1".to_string(),
                status: "Idle".to_string(),
            }]))
            .unwrap();

        let resp = handle.join().unwrap().unwrap();
        let text = resp.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("claude #a3f7b2c1"));
        assert!(text.contains("abc-123"));
    }

    #[test]
    fn tools_call_send_message_sends_event_with_from() {
        let (tx, rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "send_message", "arguments": {
                "from_agent_name": "claude #aaa",
                "to_agent_name": "claude #bbb",
                "message": "hello"
            }}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &tx));

        let event = rx.recv().unwrap();
        match &event.request {
            McpRequest::SendMessage {
                from_agent_name,
                to_agent_name,
                message,
            } => {
                assert_eq!(from_agent_name, "claude #aaa");
                assert_eq!(to_agent_name, "claude #bbb");
                assert_eq!(message, "hello");
            }
            _ => panic!("expected SendMessage"),
        }
        event.response_tx.send(McpResponse::MessageSent).unwrap();

        let resp = handle.join().unwrap().unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
    }

    #[test]
    fn tools_call_list_context_sends_event() {
        let (tx, rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "list_context", "arguments": {}}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &tx));

        let event = rx.recv().unwrap();
        assert!(matches!(event.request, McpRequest::ListContext));
        event
            .response_tx
            .send(McpResponse::ContextList(vec![crate::ContextDocInfo {
                name: "map".to_string(),
                description: "Project architecture map.".to_string(),
                last_updated: Some("2026-04-06".to_string()),
            }]))
            .unwrap();

        let resp = handle.join().unwrap().unwrap();
        let text = resp.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("map"));
        assert!(text.contains("Project architecture map."));
    }

    #[test]
    fn tools_call_get_context_sends_event() {
        let (tx, rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "get_context", "arguments": {"name": "principles"}}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &tx));

        let event = rx.recv().unwrap();
        match &event.request {
            McpRequest::GetContext { name } => assert_eq!(name, "principles"),
            _ => panic!("expected GetContext"),
        }
        event
            .response_tx
            .send(McpResponse::ContextDoc(
                "# Design Principles\nContent here.".to_string(),
            ))
            .unwrap();

        let resp = handle.join().unwrap().unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Design Principles"));
    }

    #[test]
    fn get_context_requires_name() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request(
            "tools/call",
            json!({"name": "get_context", "arguments": {}}),
        );
        let resp = handle_request(&req, &tx).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("name"));
    }

    #[test]
    fn unknown_method_returns_error() {
        let (tx, _rx) = mpsc::channel();
        let req = make_request("nonexistent/method", json!({}));
        let resp = handle_request(&req, &tx).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }
}

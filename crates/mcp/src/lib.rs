pub mod doctor;
mod handler;
pub(crate) mod jsonrpc;

use serde::Serialize;
use std::sync::mpsc;
use std::thread;

/// A request from the MCP server to the main loop.
pub struct McpEvent {
    pub request: McpRequest,
    pub response_tx: mpsc::Sender<McpResponse>,
}

pub enum McpRequest {
    ListAgents,
    SendMessage {
        from_agent_name: String,
        to_agent_name: String,
        message: String,
    },
    ListContext,
    GetContext {
        name: String,
    },
}

pub enum McpResponse {
    Agents(Vec<AgentInfo>),
    MessageSent,
    ContextList(Vec<ContextDocInfo>),
    ContextDoc(String),
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextDocInfo {
    pub name: String,
    pub description: String,
    pub last_updated: Option<String>,
}

/// Result of starting the MCP server.
pub struct McpServer {
    pub port: u16,
    pub rx: mpsc::Receiver<McpEvent>,
}

/// Start the MCP HTTP server on the given port (0 = OS picks a free port).
/// Returns the assigned port and a receiver for McpEvents.
pub fn start_mcp_server(port: u16) -> anyhow::Result<McpServer> {
    let addr = format!("127.0.0.1:{}", port);
    let server = tiny_http::Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("MCP server failed to bind to {}: {}", addr, e))?;

    let assigned_port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        _ => port,
    };

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            // Only accept POST
            if request.method() != &tiny_http::Method::Post {
                let resp = tiny_http::Response::from_string("Method not allowed")
                    .with_status_code(405)
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..])
                            .unwrap(),
                    );
                if let Err(e) = request.respond(resp) {
                    tracing::warn!(error = %e, "failed to send HTTP response");
                }
                continue;
            }

            // Read body
            let mut body = String::new();
            if request.as_reader().read_to_string(&mut body).is_err() {
                let resp = tiny_http::Response::from_string("Bad request").with_status_code(400);
                if let Err(e) = request.respond(resp) {
                    tracing::warn!(error = %e, "failed to send HTTP response");
                }
                continue;
            }

            // Parse JSON-RPC request
            let rpc_request = match serde_json::from_str::<jsonrpc::Request>(&body) {
                Ok(r) => r,
                Err(_) => {
                    let err = jsonrpc::Response::error(None, -32700, "Parse error".to_string());
                    let json = serde_json::to_string(&err).unwrap_or_default();
                    let resp = tiny_http::Response::from_string(&json).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    if let Err(e) = request.respond(resp) {
                        tracing::warn!(error = %e, "failed to send HTTP response");
                    }
                    continue;
                }
            };

            // Handle the request
            let rpc_response = handler::handle_request(&rpc_request, &tx);

            // Send HTTP response
            match rpc_response {
                Some(rpc_resp) => {
                    let json = serde_json::to_string(&rpc_resp).unwrap_or_default();
                    let resp = tiny_http::Response::from_string(&json).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    if let Err(e) = request.respond(resp) {
                        tracing::warn!(error = %e, "failed to send HTTP response");
                    }
                }
                None => {
                    // Notification — respond with 204 No Content
                    let resp = tiny_http::Response::empty(204);
                    if let Err(e) = request.respond(resp) {
                        tracing::warn!(error = %e, "failed to send HTTP response");
                    }
                }
            }
        }
    });

    Ok(McpServer {
        port: assigned_port,
        rx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_binds_and_returns_port() {
        let mcp = start_mcp_server(0).unwrap();
        assert!(mcp.port > 0);
        drop(mcp.rx);
    }
}

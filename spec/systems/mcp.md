# MCP Server — Technical Specification

## Server Setup

```rust
pub fn start_mcp_server(port: u16) -> anyhow::Result<McpServer>
```

Binds `tiny_http::Server` to `127.0.0.1:<port>`. Port 0 lets the OS pick a free port. Returns the actual assigned port and a channel receiver.

## Types

```rust
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
}

pub enum McpResponse {
    Agents(Vec<AgentInfo>),
    MessageSent,
    Error(String),
}

pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: String,
}
```

## JSON-RPC Protocol

Accepts only `POST` requests. Non-POST returns 405.

Request parsing uses:
```rust
pub struct Request {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: serde_json::Value,
}
```

### list_agents

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {"name": "list_agents", "arguments": {}}
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {"content": [{"type": "text", "text": "[{\"id\":\"...\",\"name\":\"...\",\"status\":\"...\"}]"}]}
}
```

### send_message

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {"name": "send_message", "arguments": {"from": "agent-a", "to": "agent-b", "message": "..."}}
}
```

Response on success:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {"content": [{"type": "text", "text": "Message sent"}]}
}
```

### Error Codes

| Code | Meaning |
|---|---|
| -32700 | Parse error (invalid JSON) |
| -32601 | Method not found |
| -32602 | Invalid params (missing required fields) |

## .mcp.json Auto-Discovery

Written to project root on startup, deleted on shutdown:

```json
{
  "mcpServers": {
    "auriga": {
      "type": "http",
      "url": "http://127.0.0.1:<port>",
      "autoApprove": ["list_agents", "send_message"]
    }
  }
}
```

## Request Flow

```
HTTP POST → tiny_http server thread
  → parse JSON-RPC body
  → handler::handle_request() translates method+params → McpRequest
  → create one-shot (tx, rx) channel
  → send McpEvent { request, response_tx: tx } to main thread channel
  → rx.recv() blocks until main thread responds
  → serialize McpResponse to JSON-RPC Response
  → return as HTTP response
```

Main thread side (in `poll_mcp_requests()`):
```
recv McpEvent →
  match request {
    ListAgents → collect AgentInfo from AgentStore → send Agents(vec)
    SendMessage → find target agent by name → write to PTY → send MessageSent
    _ → send Error(msg)
  }
```

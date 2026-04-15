//! Integration tests for auriga-mcp public API

use auriga_mcp::{start_mcp_server, AgentInfo, McpRequest, McpResponse};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[test]
fn server_starts_on_random_port() {
    let server = start_mcp_server(0).unwrap();
    assert!(server.port > 0);
}

#[test]
fn server_accepts_connections() {
    let server = start_mcp_server(0).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    // Give server time to start
    std::thread::sleep(Duration::from_millis(50));

    // Should be able to connect
    let result = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(1));

    assert!(result.is_ok());
}

#[test]
fn server_rejects_non_post_requests() {
    let server = start_mcp_server(0).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    std::thread::sleep(Duration::from_millis(50));

    let mut stream = TcpStream::connect(&addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();

    // Send GET request
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(request.as_bytes()).unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    assert!(response.contains("405") || response.contains("Method not allowed"));
}

#[test]
fn server_handles_invalid_json() {
    let server = start_mcp_server(0).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    std::thread::sleep(Duration::from_millis(50));

    let mut stream = TcpStream::connect(&addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();

    let body = "not valid json";
    let request = format!(
        "POST / HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    // Should return JSON-RPC parse error
    assert!(response.contains("Parse error") || response.contains("-32700"));
}

#[test]
fn server_handles_jsonrpc_request() {
    let server = start_mcp_server(0).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    std::thread::sleep(Duration::from_millis(50));

    let mut stream = TcpStream::connect(&addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();

    // Valid JSON-RPC initialize request
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    let request = format!(
        "POST / HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
        addr,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    // Should return a JSON-RPC response
    assert!(
        response.contains("jsonrpc") || response.contains("result") || response.contains("error")
    );
}

// -- Type tests --

#[test]
fn agent_info_serializes() {
    let info = AgentInfo {
        id: "123".to_string(),
        name: "claude #abc".to_string(),
        status: "idle".to_string(),
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("claude #abc"));
    assert!(json.contains("idle"));
}

#[test]
fn mcp_request_variants() {
    let _ = McpRequest::ListAgents;
    let _ = McpRequest::SendMessage {
        from_agent_name: "agent1".to_string(),
        to_agent_name: "agent2".to_string(),
        message: "hello".to_string(),
    };
}

#[test]
fn mcp_response_variants() {
    let _ = McpResponse::Agents(vec![]);
    let _ = McpResponse::MessageSent;
    let _ = McpResponse::Error("test error".to_string());
}

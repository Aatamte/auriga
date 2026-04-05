use serde::{Deserialize, Serialize};

/// An incoming JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// An outgoing JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl Response {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError { code, message }),
        }
    }
}

impl Request {
    /// Returns true if this is a notification (no id, no response expected).
    #[allow(dead_code)]
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_request_with_id() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn parse_notification_no_id() {
        let raw = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert!(req.is_notification());
        assert_eq!(req.method, "notifications/initialized");
    }

    #[test]
    fn parse_request_missing_params_defaults() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(req.params, serde_json::Value::Null);
    }

    #[test]
    fn success_response_serializes() {
        let resp = Response::success(Some(json!(1)), json!({"tools": []}));
        let s = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert!(v["result"]["tools"].is_array());
        assert!(v.get("error").is_none());
    }

    #[test]
    fn error_response_serializes() {
        let resp = Response::error(Some(json!(2)), -32601, "Method not found".to_string());
        let s = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["error"]["code"], -32601);
        assert_eq!(v["error"]["message"], "Method not found");
        assert!(v.get("result").is_none());
    }

    #[test]
    fn notification_response_omits_id() {
        let resp = Response::success(None, json!({}));
        let s = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("id").is_none());
    }
}

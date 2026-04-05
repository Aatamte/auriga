use crate::jsonrpc::{Request, Response};
use orchestrator_classifier::config::ClassifierConfig;
use orchestrator_classifier::ClassifierStatus;
use orchestrator_core::TraceId;
use orchestrator_storage::Database;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use uuid::Uuid;

/// Events sent from the doctor MCP server to the main loop.
pub struct DoctorEvent {
    pub request: DoctorRequest,
    pub response_tx: mpsc::Sender<DoctorResponse>,
}

pub enum DoctorRequest {
    ListClassifiers,
    ReloadClassifiers,
}

pub enum DoctorResponse {
    Classifiers(Vec<ClassifierStatus>),
    Ok,
}

/// Result of starting the doctor MCP server.
pub struct DoctorMcpServer {
    pub port: u16,
    pub rx: mpsc::Receiver<DoctorEvent>,
}

/// Start a doctor-specific MCP server with trace/classifier analysis tools.
/// The server reads directly from SQLite for trace data and sends events
/// to the main loop for classifier registry state.
pub fn start_doctor_mcp(
    db_path: &Path,
    classifiers_dir: PathBuf,
) -> anyhow::Result<DoctorMcpServer> {
    let db = Database::open(db_path)?;
    std::fs::create_dir_all(&classifiers_dir)?;
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| anyhow::anyhow!("Doctor MCP server failed to bind: {}", e))?;

    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        _ => 0,
    };

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            if request.method() != &tiny_http::Method::Post {
                let resp =
                    tiny_http::Response::from_string("Method not allowed").with_status_code(405);
                let _ = request.respond(resp);
                continue;
            }

            let mut body = String::new();
            if request.as_reader().read_to_string(&mut body).is_err() {
                let resp = tiny_http::Response::from_string("Bad request").with_status_code(400);
                let _ = request.respond(resp);
                continue;
            }

            let rpc_request = match serde_json::from_str::<Request>(&body) {
                Ok(r) => r,
                Err(_) => {
                    let err = Response::error(None, -32700, "Parse error".to_string());
                    let json = serde_json::to_string(&err).unwrap_or_default();
                    let resp = tiny_http::Response::from_string(&json).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/json"[..],
                        )
                        .unwrap(),
                    );
                    let _ = request.respond(resp);
                    continue;
                }
            };

            let rpc_response = handle_request(&rpc_request, &db, &tx, &classifiers_dir);

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
                    let _ = request.respond(resp);
                }
                None => {
                    let resp = tiny_http::Response::empty(204);
                    let _ = request.respond(resp);
                }
            }
        }
    });

    Ok(DoctorMcpServer { port, rx })
}

fn handle_request(
    req: &Request,
    db: &Database,
    event_tx: &mpsc::Sender<DoctorEvent>,
    classifiers_dir: &Path,
) -> Option<Response> {
    match req.method.as_str() {
        "initialize" => Some(handle_initialize(req)),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_tools_list(req)),
        "tools/call" => Some(handle_tools_call(req, db, event_tx, classifiers_dir)),
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
                "name": "orchestrator-doctor",
                "version": "0.1.0"
            },
            "instructions": "You are the doctor agent for an AI agent orchestrator. You have tools to inspect agent traces, turns, and classifier results stored in the orchestrator's database. Use these tools to analyze agent behavior, identify patterns, and diagnose issues."
        }),
    )
}

fn handle_tools_list(req: &Request) -> Response {
    Response::success(
        req.id.clone(),
        json!({
            "tools": [
                {
                    "name": "list_traces",
                    "description": "List recent agent traces. Each trace represents a single agent session with token usage, turn count, status, and model info.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": {
                                "type": "integer",
                                "description": "Maximum number of traces to return (default: 20)",
                                "default": 20
                            },
                            "offset": {
                                "type": "integer",
                                "description": "Number of traces to skip (default: 0)",
                                "default": 0
                            }
                        },
                        "required": []
                    }
                },
                {
                    "name": "get_trace",
                    "description": "Get full detail on a single trace including all its turns (user messages, assistant responses, tool use) and any classification results.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "trace_id": {
                                "type": "string",
                                "description": "The UUID of the trace to retrieve"
                            }
                        },
                        "required": ["trace_id"]
                    }
                },
                {
                    "name": "list_classifiers",
                    "description": "List all registered classifiers with their trigger type and enabled/disabled status.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "create_classifier",
                    "description": "Create a new classifier config. Writes a .json file and registers it in the orchestrator. The config defines the classifier name, trigger, and the set of labels the ML model can output.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "config": {
                                "type": "object",
                                "description": "The classifier config object with fields: name, description, version, enabled, trigger, type (ml/llm), runtime (type-specific config), labels",
                                "properties": {
                                    "name": { "type": "string" },
                                    "description": { "type": "string" },
                                    "version": { "type": "string" },
                                    "enabled": { "type": "boolean" },
                                    "trigger": { "description": "String ('incremental'/'on_complete'/'both') or object: {on, tools?: [...], tool_error?: bool}" },
                                    "type": { "type": "string", "enum": ["ml", "llm", "cli"], "default": "ml" },
                                    "runtime": { "type": "object", "description": "Type-specific runtime config. ml: {\"model\": \"name\"}, cli: {\"command\": \"prog\", \"args\": [...]}" },
                                    "labels": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": { "type": "string" },
                                                "notification": {
                                                    "type": "object",
                                                    "properties": {
                                                        "message": { "type": "string" }
                                                    },
                                                    "required": ["message"]
                                                }
                                            },
                                            "required": ["label", "notification"]
                                        }
                                    }
                                },
                                "required": ["name", "description", "version", "enabled", "trigger", "labels"]
                            }
                        },
                        "required": ["config"]
                    }
                },
                {
                    "name": "label_trace",
                    "description": "Assign a training label at a specific turn position in a trace. The label applies to the state of the trace up to and including that turn. A trace can have labels at multiple positions and for different classifiers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "trace_id": {
                                "type": "string",
                                "description": "The UUID of the trace to label"
                            },
                            "classifier_name": {
                                "type": "string",
                                "description": "The classifier this label is for"
                            },
                            "position": {
                                "type": "integer",
                                "description": "The turn number (position) to label at. The label applies to the trace state up to this turn."
                            },
                            "label": {
                                "type": "string",
                                "description": "The label to assign (must be from the classifier's label set)"
                            }
                        },
                        "required": ["trace_id", "classifier_name", "position", "label"]
                    }
                },
                {
                    "name": "list_training_labels",
                    "description": "List all training labels for a classifier. Shows which traces have been labeled and what label they were given.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "classifier_name": {
                                "type": "string",
                                "description": "The classifier to list labels for"
                            }
                        },
                        "required": ["classifier_name"]
                    }
                },
                {
                    "name": "train_classifier",
                    "description": "Train an ML classifier using its labeled traces. Loads all labeled training data, extracts features, trains a decision tree model, and saves it. Returns accuracy and label distribution stats. Only works for classifiers with type 'ml'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "classifier_name": {
                                "type": "string",
                                "description": "The classifier to train"
                            }
                        },
                        "required": ["classifier_name"]
                    }
                }
            ]
        }),
    )
}

fn handle_tools_call(
    req: &Request,
    db: &Database,
    event_tx: &mpsc::Sender<DoctorEvent>,
    classifiers_dir: &Path,
) -> Response {
    let tool_name = req
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arguments = req.params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "list_traces" => {
            let limit = arguments
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(20) as usize;
            let offset = arguments
                .get("offset")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            match db.list_traces(limit, offset) {
                Ok(traces) => {
                    let items: Vec<serde_json::Value> = traces
                        .iter()
                        .map(|t| {
                            json!({
                                "id": t.id.0.to_string(),
                                "agent_id": t.agent_id.0.to_string(),
                                "session_id": t.session_id,
                                "status": format!("{:?}", t.status),
                                "started_at": t.started_at,
                                "completed_at": t.completed_at,
                                "turn_count": t.turn_count,
                                "token_usage": {
                                    "input_tokens": t.token_usage.input_tokens,
                                    "output_tokens": t.token_usage.output_tokens,
                                    "cache_creation_input_tokens": t.token_usage.cache_creation_input_tokens,
                                    "cache_read_input_tokens": t.token_usage.cache_read_input_tokens,
                                },
                                "provider": t.provider,
                                "model": t.model,
                            })
                        })
                        .collect();
                    let text = serde_json::to_string_pretty(&items).unwrap_or_default();
                    tool_success(req, &text)
                }
                Err(e) => tool_error(req, &format!("Failed to list traces: {}", e)),
            }
        }
        "get_trace" => {
            let trace_id_str = match arguments.get("trace_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_error(req, "Missing required parameter: trace_id"),
            };
            let uuid = match Uuid::parse_str(trace_id_str) {
                Ok(u) => u,
                Err(_) => return tool_error(req, "Invalid trace_id: must be a valid UUID"),
            };
            let trace_id = TraceId(uuid);

            match db.load_trace(trace_id) {
                Ok(Some(trace)) => {
                    let turns = db.load_turns(trace_id).unwrap_or_default();
                    let classifications = db.load_classifications(trace_id).unwrap_or_default();

                    let turn_items: Vec<serde_json::Value> = turns
                        .iter()
                        .map(|t| {
                            json!({
                                "number": t.number,
                                "role": format!("{:?}", t.role),
                                "message_type": format!("{:?}", t.message_type),
                                "timestamp": t.timestamp,
                                "content": format!("{:?}", t.content),
                                "meta": format!("{:?}", t.meta),
                            })
                        })
                        .collect();

                    let classification_items: Vec<serde_json::Value> = classifications
                        .iter()
                        .map(|c| {
                            json!({
                                "classifier_name": c.classifier_name,
                                "timestamp": c.timestamp,
                                "payload": c.payload,
                            })
                        })
                        .collect();

                    let result = json!({
                        "trace": {
                            "id": trace.id.0.to_string(),
                            "agent_id": trace.agent_id.0.to_string(),
                            "session_id": trace.session_id,
                            "status": format!("{:?}", trace.status),
                            "started_at": trace.started_at,
                            "completed_at": trace.completed_at,
                            "turn_count": trace.turn_count,
                            "token_usage": {
                                "input_tokens": trace.token_usage.input_tokens,
                                "output_tokens": trace.token_usage.output_tokens,
                                "cache_creation_input_tokens": trace.token_usage.cache_creation_input_tokens,
                                "cache_read_input_tokens": trace.token_usage.cache_read_input_tokens,
                            },
                            "provider": trace.provider,
                            "model": trace.model,
                        },
                        "turns": turn_items,
                        "classifications": classification_items,
                    });
                    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
                    tool_success(req, &text)
                }
                Ok(None) => tool_error(req, &format!("Trace not found: {}", trace_id_str)),
                Err(e) => tool_error(req, &format!("Failed to load trace: {}", e)),
            }
        }
        "list_classifiers" => {
            let (response_tx, response_rx) = mpsc::channel();
            let event = DoctorEvent {
                request: DoctorRequest::ListClassifiers,
                response_tx,
            };

            if event_tx.send(event).is_err() {
                return tool_error(req, "Orchestrator is shutting down");
            }

            match response_rx.recv() {
                Ok(DoctorResponse::Classifiers(classifiers)) => {
                    let items: Vec<serde_json::Value> = classifiers
                        .iter()
                        .map(|c| {
                            json!({
                                "name": c.name,
                                "trigger": c.trigger.display_name(),
                                "enabled": c.enabled,
                            })
                        })
                        .collect();
                    let text = serde_json::to_string_pretty(&items).unwrap_or_default();
                    tool_success(req, &text)
                }
                Ok(_) => tool_error(req, "Unexpected response from orchestrator"),
                Err(_) => tool_error(req, "Failed to get response from orchestrator"),
            }
        }
        "create_classifier" => {
            let config_json = match arguments.get("config") {
                Some(v) => v,
                None => return tool_error(req, "Missing required parameter: config"),
            };

            let config: ClassifierConfig = match serde_json::from_value(config_json.clone()) {
                Ok(c) => c,
                Err(e) => return tool_error(req, &format!("Invalid classifier config: {}", e)),
            };

            // Check if a config with this name already exists
            let file_path = classifiers_dir.join(format!("{}.json", config.name));
            if file_path.exists() {
                return tool_error(req, &format!("Classifier '{}' already exists", config.name));
            }

            // Write the config file
            if let Err(e) = orchestrator_classifier::config::save_config(&file_path, &config) {
                return tool_error(req, &format!("Failed to write config: {}", e));
            }

            // Tell the main loop to reload classifiers
            let (response_tx, response_rx) = mpsc::channel();
            let event = DoctorEvent {
                request: DoctorRequest::ReloadClassifiers,
                response_tx,
            };
            if event_tx.send(event).is_err() {
                return tool_error(req, "Orchestrator is shutting down");
            }
            let _ = response_rx.recv();

            tool_success(
                req,
                &format!(
                    "Classifier '{}' created at {}",
                    config.name,
                    file_path.display()
                ),
            )
        }
        "label_trace" => {
            let trace_id_str = match arguments.get("trace_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return tool_error(req, "Missing required parameter: trace_id"),
            };
            let classifier_name = match arguments.get("classifier_name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return tool_error(req, "Missing required parameter: classifier_name"),
            };
            let position = match arguments.get("position").and_then(|v| v.as_u64()) {
                Some(p) => p as u32,
                None => return tool_error(req, "Missing required parameter: position"),
            };
            let label = match arguments.get("label").and_then(|v| v.as_str()) {
                Some(l) => l,
                None => return tool_error(req, "Missing required parameter: label"),
            };

            let uuid = match Uuid::parse_str(trace_id_str) {
                Ok(u) => u,
                Err(_) => return tool_error(req, "Invalid trace_id: must be a valid UUID"),
            };
            let trace_id = TraceId(uuid);

            // Verify the trace exists
            match db.load_trace(trace_id) {
                Ok(Some(_)) => {}
                Ok(None) => return tool_error(req, &format!("Trace not found: {}", trace_id_str)),
                Err(e) => return tool_error(req, &format!("Failed to load trace: {}", e)),
            }

            if let Err(e) = db.save_training_label(trace_id, classifier_name, position, label) {
                return tool_error(req, &format!("Failed to save label: {}", e));
            }

            tool_success(
                req,
                &format!(
                    "Labeled trace {} at position {} as '{}' for classifier '{}'",
                    trace_id_str, position, label, classifier_name
                ),
            )
        }
        "list_training_labels" => {
            let classifier_name = match arguments.get("classifier_name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return tool_error(req, "Missing required parameter: classifier_name"),
            };

            match db.load_training_labels(classifier_name) {
                Ok(labels) => {
                    let items: Vec<serde_json::Value> = labels
                        .iter()
                        .map(|(trace_id, position, label)| {
                            json!({
                                "trace_id": trace_id.0.to_string(),
                                "position": position,
                                "label": label,
                            })
                        })
                        .collect();

                    // Also compute label distribution
                    let mut distribution: std::collections::HashMap<&str, usize> =
                        std::collections::HashMap::new();
                    for (_, _, label) in &labels {
                        *distribution.entry(label.as_str()).or_insert(0) += 1;
                    }

                    let result = json!({
                        "classifier_name": classifier_name,
                        "total": labels.len(),
                        "distribution": distribution,
                        "labels": items,
                    });
                    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
                    tool_success(req, &text)
                }
                Err(e) => tool_error(req, &format!("Failed to load labels: {}", e)),
            }
        }
        "train_classifier" => {
            let classifier_name = match arguments.get("classifier_name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return tool_error(req, "Missing required parameter: classifier_name"),
            };

            // Load labeled training data
            let labels = match db.load_training_labels(classifier_name) {
                Ok(l) => l,
                Err(e) => return tool_error(req, &format!("Failed to load labels: {}", e)),
            };

            if labels.is_empty() {
                return tool_error(
                    req,
                    &format!(
                        "No training labels found for classifier '{}'. Use label_trace to add labels first.",
                        classifier_name
                    ),
                );
            }

            // Load trace + turns for each labeled example, slicing turns at position
            let mut training_data = Vec::new();
            for (trace_id, position, label) in &labels {
                let trace = match db.load_trace(*trace_id) {
                    Ok(Some(t)) => t,
                    Ok(None) => continue, // trace was deleted, skip
                    Err(_) => continue,
                };
                let all_turns = db.load_turns(*trace_id).unwrap_or_default();
                // Slice turns up to and including the labeled position
                let turns: Vec<_> = all_turns.into_iter().take(*position as usize).collect();
                training_data.push((trace, turns, label.clone()));
            }

            if training_data.len() < 2 {
                return tool_error(
                    req,
                    "Need at least 2 labeled examples to train. Add more labels.",
                );
            }

            // Determine next model version
            let next_version = match db.load_latest_model(classifier_name) {
                Ok(Some(m)) => m.version + 1,
                _ => 1,
            };

            // Train
            let params = orchestrator_ml::TrainParams::default();
            let result = match orchestrator_ml::train_decision_tree(
                &training_data,
                classifier_name,
                next_version,
                &params,
            ) {
                Ok(r) => r,
                Err(e) => return tool_error(req, &format!("Training failed: {}", e)),
            };

            // Save the model
            if let Err(e) = db.save_model(&result.model) {
                return tool_error(req, &format!("Failed to save model: {}", e));
            }

            // Tell the main loop to reload classifiers so the new model is picked up
            let (response_tx, response_rx) = mpsc::channel();
            let event = DoctorEvent {
                request: DoctorRequest::ReloadClassifiers,
                response_tx,
            };
            if event_tx.send(event).is_ok() {
                let _ = response_rx.recv();
            }

            let report = json!({
                "classifier_name": classifier_name,
                "version": next_version,
                "accuracy": result.accuracy,
                "n_train": result.n_train,
                "n_test": result.n_test,
                "label_distribution": result.label_distribution,
            });
            let text = serde_json::to_string_pretty(&report).unwrap_or_default();
            tool_success(req, &text)
        }
        _ => tool_error(req, &format!("Unknown tool: {}", tool_name)),
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
    use orchestrator_core::{AgentId, TokenUsage, Trace, TraceStatus};
    use orchestrator_storage::Database;
    use tempfile::TempDir;

    fn make_request(method: &str, params: serde_json::Value) -> Request {
        Request {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: method.to_string(),
            params,
        }
    }

    fn make_db_with_trace() -> Database {
        let db = Database::open_in_memory().unwrap();
        let trace = Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "sess-1".to_string(),
            status: TraceStatus::Complete,
            started_at: "2026-03-01T10:00:00Z".to_string(),
            completed_at: Some("2026-03-01T10:05:00Z".to_string()),
            turn_count: 2,
            token_usage: TokenUsage {
                input_tokens: 500,
                output_tokens: 200,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".to_string(),
            model: Some("claude-opus-4-6".to_string()),
        };
        db.save_trace(&trace, &[]).unwrap();
        db
    }

    #[test]
    fn initialize_returns_doctor_info() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request("initialize", json!({}));
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "orchestrator-doctor");
    }

    #[test]
    #[test]
    fn tools_list_returns_seven_tools() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request("tools/list", json!({}));
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 7);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"list_traces"));
        assert!(names.contains(&"get_trace"));
        assert!(names.contains(&"list_classifiers"));
        assert!(names.contains(&"create_classifier"));
        assert!(names.contains(&"label_trace"));
        assert!(names.contains(&"list_training_labels"));
        assert!(names.contains(&"train_classifier"));
    }

    #[test]
    fn list_traces_returns_traces() {
        let db = make_db_with_trace();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "list_traces", "arguments": {}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("sess-1"));
        assert!(text.contains("claude-opus-4-6"));
    }

    #[test]
    fn list_traces_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "list_traces", "arguments": {}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "[]");
    }

    #[test]
    fn get_trace_returns_detail() {
        let db = make_db_with_trace();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let trace_id = TraceId::from_u128(1).0.to_string();
        let req = make_request(
            "tools/call",
            json!({"name": "get_trace", "arguments": {"trace_id": trace_id}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("sess-1"));
        assert!(text.contains("turns"));
        assert!(text.contains("classifications"));
    }

    #[test]
    fn get_trace_not_found() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "get_trace", "arguments": {"trace_id": "00000000-0000-0000-0000-000000000099"}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found"));
    }

    #[test]
    fn get_trace_invalid_uuid() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "get_trace", "arguments": {"trace_id": "not-a-uuid"}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
    }

    #[test]
    fn get_trace_missing_param() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request("tools/call", json!({"name": "get_trace", "arguments": {}}));
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
    }

    #[test]
    fn list_classifiers_sends_event() {
        let db = Database::open_in_memory().unwrap();
        let (tx, rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let dir_path = dir.path().to_path_buf();
        let req = make_request(
            "tools/call",
            json!({"name": "list_classifiers", "arguments": {}}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &db, &tx, &dir_path));

        let event = rx.recv().unwrap();
        assert!(matches!(event.request, DoctorRequest::ListClassifiers));
        event
            .response_tx
            .send(DoctorResponse::Classifiers(vec![ClassifierStatus {
                name: "loop-detector".to_string(),
                trigger: orchestrator_classifier::ClassifierTrigger::new(
                    orchestrator_classifier::TriggerPhase::Both,
                    orchestrator_classifier::TurnFilter::default(),
                ),
                enabled: true,
            }]))
            .unwrap();

        let resp = handle.join().unwrap().unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("loop-detector"));
    }

    #[test]
    fn create_classifier_writes_config() {
        let db = Database::open_in_memory().unwrap();
        let (tx, rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let dir_path = dir.path().to_path_buf();
        let req = make_request(
            "tools/call",
            json!({"name": "create_classifier", "arguments": {
                "config": {
                    "name": "test-classifier",
                    "description": "A test classifier",
                    "version": "1.0",
                    "enabled": true,
                    "trigger": "on_complete",
                    "type": "ml",
                    "runtime": {"model": "test-v1"},
                    "labels": [
                        {
                            "label": "good",
                            "notification": { "message": "All good" }
                        },
                        {
                            "label": "bad",
                            "notification": { "message": "Something wrong" }
                        }
                    ]
                }
            }}),
        );

        let handle = std::thread::spawn(move || handle_request(&req, &db, &tx, &dir_path));

        // create_classifier sends a ReloadClassifiers event
        let event = rx.recv().unwrap();
        assert!(matches!(event.request, DoctorRequest::ReloadClassifiers));
        event.response_tx.send(DoctorResponse::Ok).unwrap();

        let resp = handle.join().unwrap().unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());

        // Verify the file was written
        let file_path = dir.path().join("test-classifier.json");
        assert!(file_path.exists());
        let loaded = orchestrator_classifier::config::load_config(&file_path).unwrap();
        assert_eq!(loaded.name, "test-classifier");
        assert_eq!(
            loaded.classifier_type,
            orchestrator_classifier::ClassifierType::Ml
        );
        assert_eq!(loaded.runtime["model"], "test-v1");
        assert_eq!(loaded.labels.len(), 2);
    }

    #[test]
    fn create_classifier_rejects_duplicate() {
        let db = Database::open_in_memory().unwrap();
        let (tx, rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Write an existing config
        let existing = orchestrator_classifier::config::ClassifierConfig {
            name: "existing".into(),
            description: "Already here".into(),
            version: "1.0".into(),
            enabled: true,
            trigger: orchestrator_classifier::config::ConfigTrigger::Simple(
                orchestrator_classifier::TriggerPhase::OnComplete,
            ),
            classifier_type: orchestrator_classifier::ClassifierType::Ml,
            runtime: json!({}),
            labels: vec![],
        };
        orchestrator_classifier::config::save_config(&dir.path().join("existing.json"), &existing)
            .unwrap();

        let req = make_request(
            "tools/call",
            json!({"name": "create_classifier", "arguments": {
                "config": {
                    "name": "existing",
                    "description": "Duplicate",
                    "version": "1.0",
                    "enabled": true,
                    "trigger": "on_complete",
                    "labels": []
                }
            }}),
        );

        let resp = handle_request(&req, &db, &tx, &dir_path).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("already exists"));

        // No event should have been sent
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn label_trace_stores_label_at_position() {
        let db = make_db_with_trace();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let trace_id = TraceId::from_u128(1).0.to_string();
        let req = make_request(
            "tools/call",
            json!({"name": "label_trace", "arguments": {
                "trace_id": trace_id,
                "classifier_name": "health",
                "position": 5,
                "label": "looping"
            }}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());

        let labels = db.load_training_labels("health").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].1, 5); // position
        assert_eq!(labels[0].2, "looping"); // label
    }

    #[test]
    fn label_trace_not_found() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "label_trace", "arguments": {
                "trace_id": "00000000-0000-0000-0000-000000000099",
                "classifier_name": "health",
                "position": 1,
                "label": "looping"
            }}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
    }

    #[test]
    fn label_trace_multiple_classifiers() {
        let db = make_db_with_trace();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let trace_id = TraceId::from_u128(1).0.to_string();

        // Label same trace at same position for two different classifiers
        for (clf, label) in [("health", "looping"), ("budget", "over-budget")] {
            let req = make_request(
                "tools/call",
                json!({"name": "label_trace", "arguments": {
                    "trace_id": trace_id,
                    "classifier_name": clf,
                    "position": 3,
                    "label": label
                }}),
            );
            let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
            assert!(!resp.result.unwrap()["isError"].as_bool().unwrap());
        }

        assert_eq!(db.load_training_labels("health").unwrap().len(), 1);
        assert_eq!(db.load_training_labels("budget").unwrap().len(), 1);
    }

    #[test]
    fn list_training_labels_shows_distribution() {
        let db = make_db_with_trace();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let trace_id = TraceId::from_u128(1).0.to_string();

        // Add a label
        let req = make_request(
            "tools/call",
            json!({"name": "label_trace", "arguments": {
                "trace_id": trace_id,
                "classifier_name": "health",
                "position": 2,
                "label": "healthy"
            }}),
        );
        handle_request(&req, &db, &tx, dir.path());

        // List labels
        let req = make_request(
            "tools/call",
            json!({"name": "list_training_labels", "arguments": {
                "classifier_name": "health"
            }}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(!result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("healthy"));
        assert!(text.contains("\"total\": 1"));
    }

    #[test]
    fn train_classifier_no_labels_errors() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "train_classifier", "arguments": {
                "classifier_name": "health"
            }}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No training labels"));
    }

    #[test]
    fn unknown_tool_returns_error() {
        let db = Database::open_in_memory().unwrap();
        let (tx, _rx) = mpsc::channel();
        let dir = TempDir::new().unwrap();
        let req = make_request(
            "tools/call",
            json!({"name": "nonexistent", "arguments": {}}),
        );
        let resp = handle_request(&req, &db, &tx, dir.path()).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
    }
}

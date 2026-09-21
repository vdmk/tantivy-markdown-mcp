use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{Value, json};

use crate::index::Searcher;
use crate::tools;

/// MCP protocol version advertised during initialization.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const SERVER_NAME: &str = "tantivy-markdown-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Minimal line-oriented MCP JSON-RPC server.
pub struct McpServer<S: Searcher> {
    searcher: S,
    tools_schema: Value,
}

impl<S: Searcher> McpServer<S> {
    /// Creates an MCP server using `searcher` to handle search tool calls.
    pub fn new(searcher: S) -> Self {
        Self {
            searcher,
            tools_schema: tools::schema(),
        }
    }

    /// Reads JSON-RPC requests from stdin and writes responses to stdout.
    ///
    /// Notifications are processed without emitting a response.
    pub fn run(&self) -> Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut out = stdout.lock();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            if let Some(response) = self.response_for_line(&line) {
                writeln!(out, "{}", serde_json::to_string(&response)?)?;
                out.flush()?;
            }
        }
        Ok(())
    }

    fn response_for_line(&self, line: &str) -> Option<Value> {
        let req: Value = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(_) => {
                return Some(err(
                    Value::Null,
                    json!({ "code": -32700, "message": "Parse error" }),
                ));
            }
        };

        if !is_valid_request(&req) {
            return Some(err(
                Value::Null,
                json!({ "code": -32600, "message": "Invalid Request" }),
            ));
        }

        // Notifications have no "id" — never respond to them.
        req.get("id").map(|_| self.handle(&req))
    }

    fn handle(&self, req: &Value) -> Value {
        let id = req["id"].clone();
        let method = req["method"].as_str().unwrap_or("");

        match method {
            "ping" => ok(id, json!({})),

            "initialize" => ok(
                id,
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
                }),
            ),

            "tools/list" => ok(id, json!({ "tools": &self.tools_schema })),

            "tools/call" => {
                let Some(params) = req["params"].as_object() else {
                    return make_invalid_params_error(id, "tools/call params must be an object");
                };
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return make_invalid_params_error(id, "tools/call name must be a string");
                };
                let args = params.get("arguments").unwrap_or(&Value::Null);
                if !args.is_null() && !args.is_object() {
                    return make_invalid_params_error(id, "tools/call arguments must be an object");
                }
                let (text, is_error) = match tools::call(&self.searcher, name, args) {
                    Ok(text) => (text, false),
                    Err(text) => (text, true),
                };
                ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": is_error
                    }),
                )
            }

            _ => err(id, json!({ "code": -32601, "message": "Method not found" })),
        }
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, error: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

fn make_invalid_params_error(id: Value, message: &str) -> Value {
    err(id, json!({ "code": -32602, "message": message }))
}

fn is_valid_request(req: &Value) -> bool {
    let Some(req) = req.as_object() else {
        return false;
    };

    req.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
        && req.get("method").is_some_and(Value::is_string)
        && req
            .get("params")
            .is_none_or(|params| params.is_object() || params.is_array())
        && req
            .get("id")
            .is_none_or(|id| id.is_null() || id.is_string() || id.is_number())
}

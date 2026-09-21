use anyhow::Result;
use serde_json::{Value, json};

use crate::index::Searcher;

const MAX_LIMIT: u64 = 1_000;
const MAX_SNIPPET_SIZE: u64 = 10_000;

/// Returns the JSON schema advertised for this server's MCP tools.
pub fn schema() -> Value {
    serde_json::from_str(include_str!("tools.json")).expect("tools.json is invalid")
}

/// Dispatches an MCP tool call to the supplied searcher.
pub fn call<S: Searcher>(searcher: &S, name: &str, args: &Value) -> Result<String, String> {
    match name {
        "search" => {
            let query = args["query"].as_str().unwrap_or("");
            if query.is_empty() {
                return Err("query must not be empty".to_string());
            }
            let limit = optional_integer(args, "limit", 5, MAX_LIMIT)?;
            let snippet_size = optional_integer(args, "snippet_size", 500, MAX_SNIPPET_SIZE)?;
            searcher
                .search(query, limit as usize, snippet_size as usize)
                .map_err(|e| format!("search error: {e}"))
                .map(|results| {
                    let arr: Vec<Value> = results
                        .into_iter()
                        .map(|r| {
                            json!({
                                "path": r.path,
                                "score": r.score,
                                "snippet": r.snippet,
                            })
                        })
                        .collect();
                    serde_json::to_string_pretty(&arr).unwrap()
                })
        }

        _ => Err(format!("unknown tool: {name}")),
    }
}

fn optional_integer(args: &Value, name: &str, default: u64, maximum: u64) -> Result<u64, String> {
    let Some(value) = args.get(name) else {
        return Ok(default);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{name} must be a positive integer"))?;
    if value == 0 {
        return Err(format!("{name} must be at least 1"));
    }
    if value > maximum {
        return Err(format!("{name} must not exceed {maximum}"));
    }
    Ok(value)
}

mod support;

use serde_json::json;
use tempfile::TempDir;

use support::{McpClient, fixture_knowledge_base};

#[test]
fn mcp_protocol_supports_initialize_tools_list_and_search() {
    let root = fixture_knowledge_base();
    let mut client = McpClient::start(root.path());

    let initialize = client.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}));
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "tantivy-markdown-mcp"
    );

    let tools = client.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    assert_eq!(tools["result"]["tools"][0]["name"], "search");

    let results = client.search("writer lock", 5);
    assert_eq!(results.len(), 1);
    assert!(
        results[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("architecture.md")
    );
}

#[test]
fn malformed_requests_and_tool_arguments_return_protocol_errors() {
    let root = TempDir::new().unwrap();
    let mut client = McpClient::start(root.path());

    let parse_error = client.raw_request("{not json");
    assert_eq!(parse_error["error"]["code"], -32700);

    let invalid_arguments = client.request(json!({
        "jsonrpc":"2.0",
        "id":2,
        "method":"tools/call",
        "params":{"name":"search","arguments":{"query":""}}
    }));
    assert_eq!(invalid_arguments["result"]["isError"], true);
    assert_eq!(
        invalid_arguments["result"]["content"][0]["text"],
        "query must not be empty"
    );

    let invalid_shape = client.request(json!({
        "jsonrpc":"2.0",
        "id":3,
        "method":"tools/call",
        "params":{"name":"search","arguments":[]}
    }));
    assert_eq!(invalid_shape["error"]["code"], -32602);

    let missing_name = client.request(json!({
        "jsonrpc":"2.0",
        "id":4,
        "method":"tools/call",
        "params":{"arguments":{}}
    }));
    assert_eq!(missing_name["error"]["code"], -32602);
}

#[test]
fn unknown_method_returns_method_not_found() {
    let root = TempDir::new().unwrap();
    let mut client = McpClient::start(root.path());

    let response = client.request(json!({
        "jsonrpc":"2.0",
        "id":3,
        "method":"not-a-real-method"
    }));

    assert_eq!(response["error"]["code"], -32601);
    assert_eq!(response["error"]["message"], "Method not found");
}

#[test]
fn ping_returns_empty_result() {
    let root = TempDir::new().unwrap();
    let mut client = McpClient::start(root.path());

    let response = client.request(json!({"jsonrpc":"2.0","id":"ping-1","method":"ping"}));

    assert_eq!(response["id"], "ping-1");
    assert_eq!(response["result"], json!({}));
}

#[test]
fn second_server_searches_in_read_only_mode_when_writer_is_locked() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("note.md"), "shared indexed content").unwrap();
    let data_home = TempDir::new().unwrap();

    let mut writer = McpClient::start_with_data_home(root.path(), &data_home);
    assert_eq!(writer.search("shared indexed", 10).len(), 1);

    let mut reader = McpClient::start_with_data_home(root.path(), &data_home);
    assert_eq!(reader.search("shared indexed", 10).len(), 1);
    let stderr = reader.shutdown();

    assert!(stderr.contains("index locked by another instance, running read-only"));
}

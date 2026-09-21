#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Output, Stdio};

use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

pub struct McpClient {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    stderr: Option<ChildStderr>,
    _data_home: Option<TempDir>,
}

impl McpClient {
    pub fn start(root: &Path) -> Self {
        let data_home = tempdir().unwrap();
        let data_home_path = data_home.path().to_path_buf();
        Self::spawn(root, &data_home_path, Some(data_home))
    }

    #[allow(dead_code)]
    pub fn start_with_data_home(root: &Path, data_home: &TempDir) -> Self {
        Self::spawn(root, data_home.path(), None)
    }

    fn spawn(root: &Path, data_home_path: &Path, data_home: impl Into<Option<TempDir>>) -> Self {
        let binary: std::ffi::OsString = std::env::var_os("MCP_TEST_BINARY")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tantivy-markdown-mcp").into());
        let mut child = Command::new(binary)
            .args(["serve", root.to_str().unwrap()])
            .env("XDG_DATA_HOME", data_home_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let stderr = child.stderr.take().unwrap();
        Self {
            child,
            input: Some(input),
            output,
            stderr: Some(stderr),
            _data_home: data_home.into(),
        }
    }

    pub fn request(&mut self, request: Value) -> Value {
        self.raw_request(&request.to_string())
    }

    pub fn raw_request(&mut self, request: &str) -> Value {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{request}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    pub fn search(&mut self, query: &str, limit: u64) -> Vec<Value> {
        let response = self.request(json!({
            "jsonrpc":"2.0",
            "id":42,
            "method":"tools/call",
            "params":{"name":"search","arguments":{"query":query,"limit":limit}}
        }));
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    pub fn shutdown(mut self) -> String {
        self.input.take();
        let _ = self.child.wait();
        let mut stderr = String::new();
        self.stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        stderr
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.input.take();
        if std::env::var_os("MCP_TEST_BINARY").is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

pub fn run_command(data_home: &Path, args: &[&str]) -> Output {
    let binary: std::ffi::OsString = std::env::var_os("MCP_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tantivy-markdown-mcp").into());
    Command::new(binary)
        .args(args)
        .env("XDG_DATA_HOME", data_home)
        .output()
        .unwrap()
}

pub fn fixture_knowledge_base() -> TempDir {
    let root = tempdir().unwrap();
    std::fs::write(
        root.path().join("architecture.md"),
        "The Tantivy writer lock protects the local search index.",
    )
    .unwrap();
    std::fs::write(root.path().join("unicode.md"), "καλημέρα κόσμε").unwrap();
    std::fs::write(root.path().join("ignored.txt"), "not indexed").unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::write(root.path().join(".git/ignored.md"), "not indexed").unwrap();
    root
}

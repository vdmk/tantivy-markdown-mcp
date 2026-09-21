mod support;

use std::path::Path;

use tempfile::TempDir;

use support::{McpClient, run_command};

fn create_index(root: &Path, data_home: &TempDir) {
    let mut client = McpClient::start_with_data_home(root, data_home);
    assert_eq!(client.search("indexed", 10).len(), 1);
}

#[test]
fn clear_path_removes_only_the_derived_project_index() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("note.md"), "indexed content").unwrap();
    let data_home = TempDir::new().unwrap();
    create_index(root.path(), &data_home);

    let indexes_dir = data_home.path().join("tantivy-markdown-mcp");
    assert!(indexes_dir.is_dir());
    let output = run_command(data_home.path(), &["clear", root.path().to_str().unwrap()]);

    assert!(output.status.success());
    assert!(indexes_dir.is_dir());
    assert_eq!(std::fs::read_dir(indexes_dir).unwrap().count(), 0);
}

#[test]
fn clear_missing_path_fails_without_removing_existing_indexes() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("note.md"), "indexed content").unwrap();
    let data_home = TempDir::new().unwrap();
    create_index(root.path(), &data_home);

    let indexes_dir = data_home.path().join("tantivy-markdown-mcp");
    let before = std::fs::read_dir(&indexes_dir).unwrap().count();
    let missing = root.path().join("does-not-exist");
    let output = run_command(data_home.path(), &["clear", missing.to_str().unwrap()]);

    assert!(!output.status.success());
    assert_eq!(std::fs::read_dir(indexes_dir).unwrap().count(), before);
}

#[test]
fn index_usage_reports_persisted_project_usage() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("note.md"), "indexed content").unwrap();
    let data_home = TempDir::new().unwrap();
    create_index(root.path(), &data_home);

    let output = run_command(data_home.path(), &["index-usage"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("Indexed projects: 1"));
    assert!(stdout.contains("Total indexed data:"));
}

#[test]
fn clear_without_path_removes_all_persisted_indexes() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    std::fs::write(first.path().join("first.md"), "first indexed content").unwrap();
    std::fs::write(second.path().join("second.md"), "second indexed content").unwrap();
    let data_home = TempDir::new().unwrap();
    create_index(first.path(), &data_home);
    create_index(second.path(), &data_home);

    let indexes_dir = data_home.path().join("tantivy-markdown-mcp");
    assert_eq!(std::fs::read_dir(&indexes_dir).unwrap().count(), 2);
    let output = run_command(data_home.path(), &["clear"]);

    assert!(output.status.success());
    assert!(!indexes_dir.exists());
}

#[test]
fn index_usage_with_empty_storage_reports_zero_projects() {
    let data_home = TempDir::new().unwrap();
    let output = run_command(data_home.path(), &["index-usage"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("Indexed projects: 0"));
    assert!(stdout.contains("Total indexed data: 0 B (0 bytes)"));
}

#[test]
fn clear_without_path_is_idempotent_when_storage_is_empty() {
    let data_home = TempDir::new().unwrap();
    let output = run_command(data_home.path(), &["clear"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("All indexes already clear:"));
}

#[test]
fn invalid_command_shapes_fail_without_touching_storage() {
    let data_home = TempDir::new().unwrap();

    let missing_command = run_command(data_home.path(), &[]);
    assert!(!missing_command.status.success());
    assert!(String::from_utf8_lossy(&missing_command.stderr).contains("Usage:"));

    let unknown_command = run_command(data_home.path(), &["unknown"]);
    assert!(!unknown_command.status.success());
    assert!(String::from_utf8_lossy(&unknown_command.stderr).contains("unrecognized subcommand"));

    let extra_clear_arg = run_command(data_home.path(), &["clear", "one", "two"]);
    assert!(!extra_clear_arg.status.success());
    assert!(String::from_utf8_lossy(&extra_clear_arg.stderr).contains("unexpected argument"));

    let extra_index_usage_arg = run_command(data_home.path(), &["index-usage", "unexpected"]);
    assert!(!extra_index_usage_arg.status.success());
    assert!(String::from_utf8_lossy(&extra_index_usage_arg.stderr).contains("unexpected argument"));
}

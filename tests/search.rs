mod support;

use tempfile::TempDir;

use support::{McpClient, fixture_knowledge_base};

#[test]
fn startup_scan_indexes_markdown_and_excludes_other_files() {
    let root = fixture_knowledge_base();
    let mut client = McpClient::start(root.path());

    let results = client.search("tantivy", 10);
    assert_eq!(results.len(), 1);
    assert!(
        results[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("architecture.md")
    );

    assert!(client.search("not indexed", 10).is_empty());
    assert_eq!(client.search("καλημέρα", 10).len(), 1);
}

#[test]
fn search_respects_limit_and_ranks_relevant_documents() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("broad.md"), "rust search tool").unwrap();
    std::fs::write(
        root.path().join("specific.md"),
        "rust rust rust rust search tool",
    )
    .unwrap();
    let mut client = McpClient::start(root.path());

    let results = client.search("rust", 1);
    assert_eq!(results.len(), 1);
    assert!(
        results[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("specific.md")
    );
}

#[test]
fn restart_scan_indexes_changes_and_removes_stale_files() {
    let root = TempDir::new().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    std::fs::write(root_path.join("original.md"), "old searchable phrase").unwrap();
    let data_home = TempDir::new().unwrap();
    let mut client = McpClient::start_with_data_home(&root_path, &data_home);

    assert_eq!(client.search("old searchable", 10).len(), 1);
    drop(client);

    let created = root_path.join("created.md");
    std::fs::write(&created, "updated phrase").unwrap();
    std::fs::remove_file(root_path.join("original.md")).unwrap();

    let mut client = McpClient::start_with_data_home(&root_path, &data_home);
    assert_eq!(client.search("updated phrase", 10).len(), 1);
    assert!(client.search("newly created", 10).is_empty());
    assert!(client.search("old searchable", 10).is_empty());
}

#[test]
fn empty_and_invalid_utf8_files_do_not_break_other_indexing() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("empty.md"), "").unwrap();
    std::fs::write(root.path().join("invalid.md"), [0xff, 0xfe]).unwrap();
    std::fs::write(root.path().join("valid.md"), "valid searchable content").unwrap();
    let mut client = McpClient::start(root.path());

    assert_eq!(client.search("valid searchable", 10).len(), 1);
    assert!(client.search("empty", 10).is_empty());
}

#[test]
fn files_larger_than_limit_are_not_indexed() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("large.md");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(100 * 1024 * 1024 + 1).unwrap();
    let mut client = McpClient::start(root.path());

    assert!(client.search("anything", 10).is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_markdown_files_are_not_indexed() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("outside.md");
    std::fs::write(&outside_file, "private symlink target content").unwrap();
    symlink(&outside_file, root.path().join("linked.md")).unwrap();

    let mut client = McpClient::start(root.path());

    assert!(client.search("private symlink target", 10).is_empty());
}

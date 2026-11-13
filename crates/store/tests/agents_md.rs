use std::fs;
use std::path::Path;

use sett_store::Store;

const SHIPPED_V1: &str = include_str!("../src/agents_v1.md");
const SHIPPED_V2: &str = include_str!("../src/agents_v2.md");
const BEGIN: &str = "<!-- sett:begin -->";
const END: &str = "<!-- sett:end -->";

fn agents(root: &Path) -> String {
    fs::read_to_string(root.join("AGENTS.md")).unwrap()
}

fn marked_block(text: &str) -> &str {
    let start = text.find(BEGIN).unwrap();
    let end = text.find(END).unwrap() + END.len();
    &text[start..end]
}

#[test]
fn missing_file_is_written_with_markers_and_template_wording() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sett");

    Store::open(&root).unwrap();

    let text = agents(&root);
    assert!(text.starts_with(BEGIN) || text.contains(&format!("\n{BEGIN}\n")));
    assert!(text.trim_end().ends_with(END));
    assert!(text.contains("templates/<slug>.md"));
}

#[test]
fn marked_block_is_replaced_and_text_outside_survives() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let stale = format!("my intro\n\n{BEGIN}\nold wording\n{END}\n\nmy outro\n");
    fs::write(root.join("AGENTS.md"), stale).unwrap();

    Store::open(root).unwrap();

    let text = agents(root);
    assert!(text.starts_with("my intro\n\n"));
    assert!(text.ends_with("\n\nmy outro\n"));
    assert!(!text.contains("old wording"));
    assert!(text.contains("templates/<slug>.md"));
}

#[test]
fn current_block_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sett");
    Store::open(&root).unwrap();
    let before = agents(&root);
    let mtime = fs::metadata(root.join("AGENTS.md"))
        .unwrap()
        .modified()
        .unwrap();
    // Read-only means any write attempt would fail the open.
    let mut perms = fs::metadata(root.join("AGENTS.md")).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(root.join("AGENTS.md"), perms).unwrap();

    Store::open(&root).unwrap();

    assert_eq!(agents(&root), before);
    assert_eq!(
        fs::metadata(root.join("AGENTS.md"))
            .unwrap()
            .modified()
            .unwrap(),
        mtime
    );
}

#[test]
fn unmarked_shipped_versions_are_replaced_wholesale() {
    for shipped in [SHIPPED_V1, SHIPPED_V2, &SHIPPED_V2.replace('\n', "\r\n")] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("AGENTS.md"), shipped).unwrap();

        Store::open(root).unwrap();

        let text = agents(root);
        assert!(text.contains(BEGIN) && text.contains("templates/<slug>.md"));
        assert_eq!(text.matches("# sett data").count(), 1);
    }
}

#[test]
fn unmarked_user_edited_file_gets_the_block_appended() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mine = "my own notes\n";
    fs::write(root.join("AGENTS.md"), mine).unwrap();

    Store::open(root).unwrap();

    let text = agents(root);
    assert!(text.starts_with("my own notes\n\n<!-- sett:begin -->\n"));
    assert!(text.trim_end().ends_with(END));
    assert_eq!(text.matches(BEGIN).count(), 1);
    let again = marked_block(&text).to_string();
    Store::open(root).unwrap();
    assert_eq!(marked_block(&agents(root)), again);
}

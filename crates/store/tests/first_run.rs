use sett_store::{Space, Store};

#[test]
fn first_run_creates_both_spaces_with_an_inbox_and_agents_md() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sett");

    let store = Store::open(&root).unwrap();

    assert!(root.join("AGENTS.md").is_file());
    for space in [Space::Personal, Space::Work] {
        let projects = store.projects(space);
        assert_eq!(projects.len(), 1);
        let inbox = &projects[0];
        assert_eq!(inbox.slug, "inbox");
        assert_eq!(inbox.name, "Inbox");
        assert_eq!(inbox.columns, ["Todo", "Doing", "Done"]);
        assert!(!inbox.archived);
    }
    assert!(root.join("personal/projects/inbox/project.md").is_file());
    assert!(root.join("work/projects/inbox/project.md").is_file());
}

#[test]
fn existing_data_root_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("AGENTS.md"), "my own notes\n").unwrap();

    let store = Store::open(root).unwrap();

    assert_eq!(
        std::fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        "my own notes\n"
    );
    assert!(store.projects(Space::Work).is_empty());
    assert!(!root.join("work").exists());
}

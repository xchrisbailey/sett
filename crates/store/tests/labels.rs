use std::fs;

use sett_store::{Date, Space, Store};

fn today() -> Date {
    Date::parse("2026-03-01").unwrap()
}

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    (dir, store)
}

fn task_path(dir: &tempfile::TempDir, slug: &str) -> std::path::PathBuf {
    dir.path()
        .join(format!("sett/work/projects/inbox/tasks/{slug}.md"))
}

fn write_task(dir: &tempfile::TempDir, slug: &str, frontmatter: &str) {
    fs::write(
        task_path(dir, slug),
        format!("---\ntitle: {slug}\ncolumn: Todo\norder: 1\n{frontmatter}---\n"),
    )
    .unwrap();
}

fn labels(store: &Store, slug: &str) -> Vec<String> {
    store.task(Space::Work, "inbox", slug).unwrap().labels
}

#[test]
fn labels_are_read_from_a_list_or_a_plain_string() {
    let (dir, store) = new_store();
    write_task(&dir, "list", "labels: [bug, Urgent]\n");
    write_task(&dir, "plain", "labels: bug\n");
    write_task(&dir, "none", "");

    assert_eq!(labels(&store, "list"), ["bug", "Urgent"]);
    assert_eq!(labels(&store, "plain"), ["bug"]);
    assert!(labels(&store, "none").is_empty());
}

#[test]
fn junk_blanks_and_case_duplicates_are_dropped_keeping_first_spelling_and_order() {
    let (dir, store) = new_store();
    write_task(
        &dir,
        "messy",
        "labels: [' Bug ', 3, '', '  ', Zed, bug, {a: b}, BUG, alpha]\n",
    );
    write_task(&dir, "junk", "labels: {a: b}\n");

    assert_eq!(labels(&store, "messy"), ["Bug", "Zed", "alpha"]);
    assert!(labels(&store, "junk").is_empty());
}

#[test]
fn saving_writes_a_normalised_list_and_removes_the_key_when_empty() {
    let (dir, store) = new_store();
    write_task(&dir, "t", "owner: sam\nlabels: [bug]\n");

    let mut task = store.task(Space::Work, "inbox", "t").unwrap();
    task.labels = vec![" Ship ".into(), "ship".into(), "".into(), "docs".into()];
    store.save_task(&mut task).unwrap();
    assert_eq!(labels(&store, "t"), ["Ship", "docs"]);
    let text = fs::read_to_string(task_path(&dir, "t")).unwrap();
    assert!(text.contains("labels:\n- Ship\n- docs\n"), "{text}");
    assert!(text.contains("owner: sam"), "{text}");

    task.labels.clear();
    store.save_task(&mut task).unwrap();
    let text = fs::read_to_string(task_path(&dir, "t")).unwrap();
    assert!(!text.contains("labels"), "{text}");
    assert!(text.contains("owner: sam"), "{text}");
}

#[test]
fn set_task_labels_writes_only_that_task() {
    let (dir, store) = new_store();
    write_task(&dir, "a", "");
    write_task(&dir, "b", "");
    let untouched = fs::read_to_string(task_path(&dir, "b")).unwrap();

    let task = store
        .set_task_labels(Space::Work, "inbox", "a", &["bug".into(), "BUG".into()])
        .unwrap();

    assert_eq!(task.labels, ["bug"]);
    assert_eq!(labels(&store, "a"), ["bug"]);
    assert_eq!(fs::read_to_string(task_path(&dir, "b")).unwrap(), untouched);
    assert!(
        store
            .set_task_labels(Space::Work, "inbox", "missing", &[])
            .is_err()
    );
}

#[test]
fn create_task_in_writes_the_label_when_given() {
    let (_dir, store) = new_store();

    let with = store
        .create_task_in(Space::Work, "inbox", "Todo", "Fix", today(), Some(" bug "))
        .unwrap();
    let without = store
        .create_task_in(Space::Work, "inbox", "Todo", "Other", today(), None)
        .unwrap();

    assert_eq!(with.labels, ["bug"]);
    assert_eq!(labels(&store, "fix"), ["bug"]);
    assert!(without.labels.is_empty());
}

#[test]
fn a_projects_labels_are_deduped_ignoring_case_and_sorted() {
    let (dir, store) = new_store();
    write_task(&dir, "a", "labels: [zed, Bug]\n");
    write_task(&dir, "b", "labels: [BUG, alpha]\ndone: 2026-01-01\n");

    let tasks = store.tasks(Space::Work, "inbox");
    let found = sett_store::labels_of(&tasks);

    assert_eq!(found.len(), 3);
    assert_eq!(found, ["alpha", "Bug", "zed"]);
}

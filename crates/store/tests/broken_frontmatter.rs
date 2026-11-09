//! A file whose frontmatter can't be read is never written by sett (ADR 0001:
//! tolerate hand edits, never "fix" the file).

use std::fs;
use std::path::PathBuf;

use sett_store::{Date, Space, Store};

const DUPLICATE_KEY: &str = "---\ntitle: A\ntitle: B\nowner: sam\n---\nBody\n";
const NOT_A_MAPPING: &str = "---\n- just\n- a list\n---\nBody\n";

fn date(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    (dir, store)
}

fn write(store: &Store, rel: &str, text: &str) -> PathBuf {
    let path = store.root().join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn broken_task_is_never_saved() {
    for text in [DUPLICATE_KEY, NOT_A_MAPPING] {
        let (_dir, store) = new_store();
        let path = write(&store, "work/projects/inbox/tasks/deploy.md", text);

        let mut task = store.task(Space::Work, "inbox", "deploy").unwrap();
        assert!(task.frontmatter_error(), "{text}");
        assert_eq!(task.body, "Body\n");
        task.body = "Edited\n".into();
        assert!(store.save_task(&mut task).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn broken_note_and_journal_entry_are_never_saved() {
    for text in [DUPLICATE_KEY, NOT_A_MAPPING] {
        let (_dir, store) = new_store();
        let note_path = write(&store, "work/notes/ideas.md", text);
        let entry_path = write(&store, "work/journal/2026-03-04.md", text);

        let mut note = store.note(Space::Work, None, "ideas").unwrap();
        assert!(note.frontmatter_error());
        note.body = "Edited\n".into();
        assert!(store.save_note(&mut note).is_err());

        let mut entry = store
            .journal_entry(Space::Work, date("2026-03-04"))
            .unwrap();
        assert!(entry.frontmatter_error());
        entry.body = "Edited\n".into();
        assert!(store.save_journal_entry(&mut entry).is_err());

        assert_eq!(fs::read_to_string(&note_path).unwrap(), text);
        assert_eq!(fs::read_to_string(&entry_path).unwrap(), text);
    }
}

#[test]
fn broken_project_is_never_saved() {
    let today = date("2026-03-04");
    for text in [DUPLICATE_KEY, NOT_A_MAPPING] {
        let (_dir, store) = new_store();
        let project = store.create_project(Space::Work, "Site", None).unwrap();
        let path = write(&store, "work/projects/site/project.md", text);
        let mut project = store.project(Space::Work, &project.slug).unwrap();
        assert!(project.frontmatter_error());

        let mut archived = project.clone();
        archived.archived = true;
        assert!(store.save_project(&mut archived).is_err());
        assert!(store.add_column(&mut project, "Review").is_err());
        assert!(
            store
                .rename_column(&mut project, [], "Todo", "Later")
                .is_err()
        );
        assert!(store.move_column(&mut project, [], 0, 1, today).is_err());
        assert!(
            store
                .create_task(Space::Work, "site", "New", today)
                .is_err(),
            "a broken Project's Columns aren't known"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        assert!(store.tasks(Space::Work, "site").is_empty());
    }
}

#[test]
fn column_ops_skip_broken_tasks() {
    let today = date("2026-03-04");
    let (_dir, store) = new_store();
    let mut project = store.create_project(Space::Work, "Site", None).unwrap();
    let ok = "---\ntitle: Ok\ncolumn: Done\norder: 1\n---\n";
    write(&store, "work/projects/site/tasks/ok.md", ok);
    let broken = "---\ntitle: A\ntitle: B\ncolumn: Done\n---\nBody\n";
    let broken_path = write(&store, "work/projects/site/tasks/broken.md", broken);

    let tasks = store.tasks(Space::Work, "site");
    store
        .rename_column(&mut project, &tasks, "Done", "Shipped")
        .unwrap();
    let tasks = store.tasks(Space::Work, "site");
    store
        .move_column(&mut project, &tasks, 2, 0, today)
        .unwrap();

    assert_eq!(fs::read_to_string(&broken_path).unwrap(), broken);
    let ok = store.task(Space::Work, "site", "ok").unwrap();
    assert_eq!(ok.column.as_deref(), Some("Shipped"));
}

#[test]
fn moving_skips_or_refuses_broken_tasks() {
    let today = date("2026-03-04");
    let (_dir, store) = new_store();
    for (slug, order) in [("a", 1.0), ("b", 1.0), ("c", 1.0)] {
        write(
            &store,
            &format!("work/projects/inbox/tasks/{slug}.md"),
            &format!("---\ntitle: {slug}\ncolumn: Todo\norder: {order}\n---\n"),
        );
    }
    let broken = "---\ntitle: x\ntitle: y\ncolumn: Todo\norder: 1\n---\n";
    let broken_path = write(&store, "work/projects/inbox/tasks/broken.md", broken);
    let project = store.project(Space::Work, "inbox").unwrap();
    let tasks = store.tasks(Space::Work, "inbox");
    let board = sett_store::Board::new(&project.columns, &tasks);

    assert!(
        store
            .move_task(&board, "broken", "Doing", None, today)
            .is_err()
    );
    // The broken Task has no order, so moving next to it renumbers the
    // Column, around the broken Task without touching it.
    let written = store
        .move_task(&board, "a", "Todo", Some("broken"), today)
        .unwrap();
    assert!(!written.is_empty());
    assert!(written.iter().all(|t| t.slug != "broken"));
    assert_eq!(fs::read_to_string(&broken_path).unwrap(), broken);
}

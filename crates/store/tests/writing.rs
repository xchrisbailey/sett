use std::fs;

use sett_store::{Date, JournalEntry, Space, Store};

fn date(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    (dir, store)
}

#[test]
fn new_items_get_slugs_from_titles_and_clashes_are_numbered() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");

    let slugs: Vec<String> = [
        "Fix the Login bug!",
        "fix the login bug",
        "  Fix -- the login BUG ",
        "???",
    ]
    .iter()
    .map(|t| {
        store
            .create_task(Space::Work, "inbox", t, today)
            .unwrap()
            .slug
    })
    .collect();
    assert_eq!(
        slugs,
        [
            "fix-the-login-bug",
            "fix-the-login-bug-2",
            "fix-the-login-bug-3",
            "untitled"
        ]
    );

    let note = store
        .create_note(Space::Work, None, "Fix the login bug", today)
        .unwrap();
    assert_eq!(
        note.slug, "fix-the-login-bug",
        "Notes don't clash with Tasks"
    );

    let project = store.create_project(Space::Work, "Inbox", None).unwrap();
    assert_eq!(project.slug, "inbox-2");
    assert_eq!(project.columns, ["Todo", "Doing", "Done"]);
    assert_eq!(project.icon.as_deref(), Some("folder"));
}

#[test]
fn new_task_lands_at_the_end_of_the_first_column() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");

    let first = store
        .create_task(Space::Personal, "inbox", "Buy milk", today)
        .unwrap();
    let second = store
        .create_task(Space::Personal, "inbox", "Walk dog", today)
        .unwrap();

    assert_eq!(first.column.as_deref(), Some("Todo"));
    assert_eq!(first.created, Some(today));
    assert!(second.order.unwrap() > first.order.unwrap());

    let loaded = store.tasks(Space::Personal, "inbox");
    assert_eq!(loaded, [first, second]);
}

#[test]
fn saving_keeps_the_slug_and_unknown_frontmatter() {
    let (dir, store) = new_store();
    let path = dir.path().join("sett/work/projects/inbox/tasks/deploy.md");
    fs::write(
        &path,
        "---\ntitle: Deploy\nowner: sam\ncolumn: Todo\norder: 1\ncreated: 2026-01-01\n---\nOld body\n",
    )
    .unwrap();

    let mut task = store.tasks(Space::Work, "inbox").remove(0);
    task.title = "Deploy to prod".into();
    task.body = "New body\n".into();
    task.due = Some(date("2026-04-01"));
    store.save_task(&mut task).unwrap();

    let reloaded = store.tasks(Space::Work, "inbox").remove(0);
    assert_eq!(reloaded, task);
    assert_eq!(reloaded.slug, "deploy");
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("owner: sam"), "{text}");
    assert!(
        !dir.path()
            .join("sett/work/projects/inbox/tasks/deploy-to-prod.md")
            .exists()
    );
}

#[test]
fn moving_a_task_into_the_last_column_marks_it_done() {
    let (_dir, store) = new_store();
    let columns = store.projects(Space::Work)[0].columns.clone();
    let mut task = store
        .create_task(Space::Work, "inbox", "Write report", date("2026-03-01"))
        .unwrap();

    task.move_to("Done", 1.0, &columns, date("2026-03-05"));
    assert_eq!(task.done, Some(date("2026-03-05")));

    task.move_to("Doing", 1.0, &columns, date("2026-03-06"));
    assert_eq!(task.done, None);
    assert_eq!(task.column.as_deref(), Some("Doing"));
}

#[test]
fn journal_entry_is_only_written_when_saved() {
    let (dir, store) = new_store();
    let day = date("2026-03-04");
    let path = dir.path().join("sett/personal/journal/2026-03-04.md");

    let mut entry = JournalEntry::new(Space::Personal, day);
    assert!(!path.exists());

    entry.body = "Good day\n".into();
    store.save_journal_entry(&mut entry).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "Good day\n");
    assert_eq!(store.journal(Space::Personal), [entry]);
}

#[test]
fn notes_round_trip_in_space_and_project() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");

    let mut note = store
        .create_note(Space::Personal, Some("inbox"), "Ideas", today)
        .unwrap();
    note.body = "Link to [[buy-milk]]\n".into();
    store.save_note(&mut note).unwrap();

    assert_eq!(store.notes(Space::Personal, Some("inbox")), [note]);
    assert!(store.notes(Space::Personal, None).is_empty());
}

#[test]
fn inbox_cannot_be_trashed() {
    let (_dir, store) = new_store();
    let inbox = store.projects(Space::Work).remove(0);
    assert!(store.trash_project(&inbox).is_err());
    assert_eq!(store.projects(Space::Work).len(), 1);
}

#[test]
fn written_task_file_is_plain_frontmatter() {
    let (dir, store) = new_store();
    store
        .create_task(Space::Work, "inbox", "Ship it", date("2026-03-04"))
        .unwrap();
    let text =
        fs::read_to_string(dir.path().join("sett/work/projects/inbox/tasks/ship-it.md")).unwrap();
    assert_eq!(
        text,
        "---\ntitle: Ship it\ncolumn: Todo\norder: 1.0\ncreated: 2026-03-04\n---\n"
    );
}

use std::fs;
use std::path::{Path, PathBuf};

use sett_store::{Board, Date, Space, Store, Task};

fn date(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    (dir, store)
}

fn tasks_dir(store: &Store) -> PathBuf {
    store.root().join("work/projects/inbox/tasks")
}

/// Writes a Task by hand in a form sett would never produce itself (a YAML
/// comment, which a rewrite drops), so a test can tell whether sett rewrote it.
fn write_task(store: &Store, slug: &str, column: &str, order: f64, done: Option<&str>) {
    let done = done.map(|d| format!("done: {d}\n")).unwrap_or_default();
    let text = format!(
        "---\n# written by hand\ntitle: {slug}\ncolumn: {column}\norder: {order}\n{done}---\nbody\n"
    );
    fs::create_dir_all(tasks_dir(store)).unwrap();
    fs::write(tasks_dir(store).join(format!("{slug}.md")), text).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn task_with_body(body: &str) -> Task {
    let (_dir, store) = new_store();
    let mut task = store
        .create_task(Space::Work, "inbox", "t", date("2026-03-04"))
        .unwrap();
    task.body = body.to_string();
    task
}

#[test]
fn checklist_counts_every_box_in_the_body() {
    let task = task_with_body(
        "Intro\n- [ ] one\n- [x] two\n  - [X] nested\n* [ ] star\nnot - [ ] mid-line\n```\n- [ ] in code\n```\n- [x] three\n",
    );
    assert_eq!(task.checklist(), Some((3, 5)));
    assert_eq!(task_with_body("No boxes\n- plain item\n").checklist(), None);
}

#[test]
fn done_tasks_count_as_recent_for_14_days() {
    let today = date("2026-03-04");
    let mut task = task_with_body("");
    assert!(task.is_recently_done(today), "no done date: still shown");
    for (done, recent) in [
        ("2026-03-04", true),
        ("2026-02-18", true),
        ("2026-02-17", false),
        ("2025-12-31", false),
    ] {
        task.done = Some(date(done));
        assert_eq!(task.is_recently_done(today), recent, "done {done}");
    }
    task.done = Some(date("2024-12-31"));
    assert!(task.is_recently_done(date("2025-01-14")), "across a year");
    assert!(!task.is_recently_done(date("2025-01-15")));
}

fn slugs(tasks: &[&Task]) -> Vec<String> {
    tasks.iter().map(|t| t.slug.clone()).collect()
}

fn inbox_board(store: &Store) -> (Vec<String>, Vec<Task>) {
    let project = store.project(Space::Work, "inbox").unwrap();
    (project.columns, store.tasks(Space::Work, "inbox"))
}

#[test]
fn board_groups_tasks_by_column_in_order_with_strays_in_the_first() {
    let (_dir, store) = new_store();
    write_task(&store, "b", "Todo", 2.0, None);
    write_task(&store, "a", "Todo", 1.0, None);
    write_task(&store, "doing", "Doing", 1.0, None);
    write_task(&store, "stray", "Someday", 0.5, None);
    let (columns, tasks) = inbox_board(&store);

    let board = Board::new(&columns, &tasks);
    let names: Vec<&str> = board.columns().iter().map(|c| c.name()).collect();
    assert_eq!(names, ["Todo", "Doing", "Done"]);
    assert_eq!(slugs(board.columns()[0].tasks()), ["a", "b", "stray"]);
    assert_eq!(slugs(board.columns()[1].tasks()), ["doing"]);
    assert!(board.columns()[2].tasks().is_empty());
}

#[test]
fn moving_a_task_rewrites_only_its_own_file() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");
    for (slug, order) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
        write_task(&store, slug, "Todo", order, None);
    }
    let path = |slug: &str| tasks_dir(&store).join(format!("{slug}.md"));
    let before: Vec<String> = ["a", "b"].iter().map(|s| read(&path(s))).collect();

    let (columns, tasks) = inbox_board(&store);
    let written = store
        .move_task(&Board::new(&columns, &tasks), "c", "Todo", Some("b"), today)
        .unwrap();
    assert_eq!(slugs(&written.iter().collect::<Vec<_>>()), ["c"]);
    assert_eq!(read(&path("a")), before[0]);
    assert_eq!(read(&path("b")), before[1]);
    let (columns, tasks) = inbox_board(&store);
    assert_eq!(
        slugs(Board::new(&columns, &tasks).columns()[0].tasks()),
        ["a", "c", "b"]
    );

    // To the end of the last Column: marks it done.
    let written = store
        .move_task(&Board::new(&columns, &tasks), "a", "Done", None, today)
        .unwrap();
    assert_eq!(written[0].done, Some(today));
    let (columns, tasks) = inbox_board(&store);
    let board = Board::new(&columns, &tasks);
    assert_eq!(slugs(board.columns()[0].tasks()), ["c", "b"]);
    assert_eq!(slugs(board.columns()[2].tasks()), ["a"]);
}

#[test]
fn column_is_renumbered_only_when_the_gap_runs_out() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");
    let tight = 1.0_f64.next_up();
    write_task(&store, "a", "Todo", 1.0, None);
    write_task(&store, "b", "Todo", tight, None);
    write_task(&store, "moved", "Doing", 1.0, None);
    write_task(&store, "doing", "Doing", 2.0, None);
    let untouched = read(&tasks_dir(&store).join("doing.md"));

    let (columns, tasks) = inbox_board(&store);
    let written = store
        .move_task(
            &Board::new(&columns, &tasks),
            "moved",
            "Todo",
            Some("b"),
            today,
        )
        .unwrap();
    let mut written = slugs(&written.iter().collect::<Vec<_>>());
    written.sort();
    // "a" already has order 1 and keeps it; only the other two are rewritten.
    assert_eq!(written, ["b", "moved"]);
    assert_eq!(read(&tasks_dir(&store).join("doing.md")), untouched);

    let (columns, tasks) = inbox_board(&store);
    let board = Board::new(&columns, &tasks);
    let todo = board.columns()[0].tasks();
    assert_eq!(slugs(todo), ["a", "moved", "b"]);
    let orders: Vec<Option<f64>> = todo.iter().map(|t| t.order).collect();
    assert_eq!(orders, [Some(1.0), Some(2.0), Some(3.0)]);
}

#[test]
fn quick_added_task_goes_to_the_top_of_its_column() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");
    write_task(&store, "doing", "Doing", 1.0, None);

    let task = store
        .create_task_in(Space::Work, "inbox", "Doing", "Write tests", today, None)
        .unwrap();
    assert_eq!(task.column.as_deref(), Some("Doing"));
    assert_eq!(task.done, None);
    let done = store
        .create_task_in(Space::Work, "inbox", "Done", "Already shipped", today, None)
        .unwrap();
    assert_eq!(done.done, Some(today), "the last Column means done");

    let (columns, tasks) = inbox_board(&store);
    let board = Board::new(&columns, &tasks);
    assert_eq!(slugs(board.columns()[1].tasks()), ["write-tests", "doing"]);
    assert!(
        store
            .create_task_in(Space::Work, "inbox", "Nope", "x", today, None)
            .is_err()
    );
}

fn inbox(store: &Store) -> sett_store::Project {
    store.project(Space::Work, "inbox").unwrap()
}

#[test]
fn new_column_goes_before_the_last_and_needs_a_unique_name() {
    let (_dir, store) = new_store();
    let mut project = inbox(&store);

    store.add_column(&mut project, "  Review ").unwrap();
    assert_eq!(project.columns, ["Todo", "Doing", "Review", "Done"]);
    assert_eq!(inbox(&store).columns, project.columns, "saved");

    for bad in ["", "   ", "doing", "Done"] {
        assert!(store.add_column(&mut project, bad).is_err(), "{bad:?}");
    }
    assert_eq!(inbox(&store).columns, ["Todo", "Doing", "Review", "Done"]);
}

#[test]
fn renaming_a_column_moves_its_tasks_with_it() {
    let (_dir, store) = new_store();
    write_task(&store, "a", "Doing", 1.0, None);
    write_task(&store, "todo", "Todo", 1.0, None);
    let untouched = read(&tasks_dir(&store).join("todo.md"));
    let mut project = inbox(&store);
    let tasks = store.tasks(Space::Work, "inbox");

    assert!(
        store
            .rename_column(&mut project, &tasks, "Doing", "todo")
            .is_err()
    );
    store
        .rename_column(&mut project, &tasks, "Doing", "In progress")
        .unwrap();
    assert_eq!(inbox(&store).columns, ["Todo", "In progress", "Done"]);
    assert_eq!(read(&tasks_dir(&store).join("todo.md")), untouched);
    let (columns, tasks) = inbox_board(&store);
    assert_eq!(
        slugs(Board::new(&columns, &tasks).columns()[1].tasks()),
        ["a"]
    );
}

#[test]
fn reordering_columns_keeps_done_in_step_with_the_last_column() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");
    write_task(&store, "todo", "Todo", 1.0, None);
    write_task(&store, "doing", "Doing", 1.0, None);
    write_task(&store, "shipped", "Done", 1.0, Some("2026-03-01"));
    let untouched = read(&tasks_dir(&store).join("todo.md"));
    let mut project = inbox(&store);
    let tasks = store.tasks(Space::Work, "inbox");

    // Swap Doing and Done: Doing becomes the last Column.
    store
        .move_column(&mut project, &tasks, 2, 1, today)
        .unwrap();
    assert_eq!(inbox(&store).columns, ["Todo", "Done", "Doing"]);
    let task = |slug| store.task(Space::Work, "inbox", slug).unwrap();
    assert_eq!(task("doing").done, Some(today));
    assert_eq!(task("shipped").done, None);
    assert_eq!(read(&tasks_dir(&store).join("todo.md")), untouched);

    assert!(
        store
            .move_column(&mut project, &tasks, 0, 3, today)
            .is_err()
    );
}

#[test]
fn only_an_empty_column_can_be_deleted_and_two_always_remain() {
    let (_dir, store) = new_store();
    let today = date("2026-03-04");
    write_task(&store, "doing", "Doing", 1.0, None);
    write_task(&store, "stray", "Someday", 1.0, None);
    let mut project = inbox(&store);
    let tasks = store.tasks(Space::Work, "inbox");
    let board = Board::new(&project.columns, &tasks);
    let deletable: Vec<bool> = ["Todo", "Doing", "Done"]
        .iter()
        .map(|c| board.is_column_deletable(c))
        .collect();
    // Todo holds the stray Task, Doing holds a Task.
    assert_eq!(deletable, [false, false, true]);
    assert!(
        store
            .delete_column(&mut project, &tasks, "Doing", today)
            .is_err()
    );

    // Deleting the last Column makes Doing last, so its Task is done now.
    store
        .delete_column(&mut project, &tasks, "Done", today)
        .unwrap();
    assert_eq!(inbox(&store).columns, ["Todo", "Doing"]);
    assert_eq!(
        store.task(Space::Work, "inbox", "doing").unwrap().done,
        Some(today)
    );

    store.add_column(&mut project, "Empty").unwrap();
    store
        .delete_column(&mut project, &tasks, "Empty", today)
        .unwrap();
    let tasks = store.tasks(Space::Work, "inbox");
    store
        .move_task(
            &Board::new(&project.columns, &tasks),
            "doing",
            "Todo",
            None,
            today,
        )
        .unwrap();
    let tasks = store.tasks(Space::Work, "inbox");
    assert!(!Board::new(&project.columns, &tasks).is_column_deletable("Doing"));
    assert!(
        store
            .delete_column(&mut project, &tasks, "Doing", today)
            .is_err(),
        "two remain"
    );
}

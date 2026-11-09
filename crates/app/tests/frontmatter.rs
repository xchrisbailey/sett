//! UI integration tests for Items and Projects whose frontmatter can't be
//! read: sett shows a notice and never writes them (ADR 0001).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Fixture, open_shell};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::Change;

const BROKEN: &str = "---\ntitle: A\ntitle: B\n---\nBody\n";

fn write(f: &Fixture, rel: &str, text: &str) -> PathBuf {
    let path = f.data_root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn apply(cx: &mut TestAppContext, f: &Fixture, change: Change, selection: Selection) {
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |m, cx| {
            m.apply_change(change, cx);
            m.select(selection, cx);
        })
    });
}

/// Types into the open body and title, then waits past autosave.
fn try_to_edit(cx: &mut TestAppContext, f: &Fixture, title: Option<&'static str>) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.input("typed", cx);
        if let Some(title) = title {
            window.click(title, cx);
            window.input("typed", cx);
        }
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
}

fn shows_notice(cx: &mut TestAppContext, f: &Fixture) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find("frontmatter-error").is_some()
    })
    .unwrap()
}

fn unchanged(path: &Path) {
    assert_eq!(fs::read_to_string(path).unwrap(), BROKEN);
}

#[gpui_kit::test]
fn a_broken_note_shows_a_notice_and_is_never_saved(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = write(&f, "personal/notes/plan.md", BROKEN);
    let change = Change::Note {
        project: None,
        slug: "plan".into(),
    };
    apply(cx, &f, change, Selection::Notes);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
    })
    .unwrap();

    assert!(shows_notice(cx, &f));
    try_to_edit(cx, &f, Some("note-title"));
    apply(cx, &f, Change::All, Selection::Journal);
    cx.run_until_parked();
    unchanged(&path);
}

#[gpui_kit::test]
fn a_broken_journal_entry_is_never_saved(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d");
    let path = write(&f, &format!("personal/journal/{today}.md"), BROKEN);
    apply(cx, &f, Change::All, Selection::Journal);

    assert!(shows_notice(cx, &f));
    try_to_edit(cx, &f, None);
    unchanged(&path);
}

#[gpui_kit::test]
fn a_broken_task_is_never_saved(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = write(&f, "personal/projects/inbox/tasks/a.md", BROKEN);
    apply(
        cx,
        &f,
        Change::Project("inbox".into()),
        Selection::Project("inbox".into()),
    );
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("card-a", cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert!(shows_notice(cx, &f));
    try_to_edit(cx, &f, Some("task-title"));
    cx.update_window(f.window, |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    unchanged(&path);
}

#[gpui_kit::test]
fn a_broken_project_shows_a_notice_and_takes_no_tasks(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = write(&f, "personal/projects/inbox/project.md", BROKEN);
    apply(
        cx,
        &f,
        Change::Project("inbox".into()),
        Selection::Project("inbox".into()),
    );

    assert!(shows_notice(cx, &f));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("quick-add").is_none());
        assert!(window.try_find("add-column").is_none());
    })
    .unwrap();
    unchanged(&path);
}

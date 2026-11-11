//! UI integration tests for the Callout type menu: opening it from the
//! header or the keyboard, picking a type or "Plain quote", closing it, and
//! undo.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use common::{Fixture, click_label, open_shell};
use gpui_kit::test::{TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::Change;

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

/// The type menu's shortcut.
const SHORTCUT: &str = if cfg!(target_os = "macos") {
    "cmd-alt-c"
} else {
    "ctrl-alt-c"
};

/// Opens a Note whose body is `body`, with the caret at the end of it.
fn open_note(cx: &mut TestAppContext, body: &str) -> (Fixture, PathBuf) {
    let f = open_shell(cx);
    let path = f.data_root.join("personal/notes/plan.md");
    fs::write(&path, format!("---\ntitle: Plan\n---\n{body}")).unwrap();
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(
                Change::Note {
                    project: None,
                    slug: "plan".into(),
                },
                cx,
            );
            w.select(Selection::Notes, cx);
        })
    });
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
    })
    .unwrap();
    cx.run_until_parked();
    (f, path)
}

fn input(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn press(cx: &mut TestAppContext, f: &Fixture, keys: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        for key in keys.split_whitespace() {
            window.press(key, cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
}

fn click(cx: &mut TestAppContext, f: &Fixture, scope: Option<&'static str>, label: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, scope, label, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn menu_open(cx: &mut TestAppContext, f: &Fixture) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label("Callout type").is_some()
    })
    .unwrap()
}

fn saved_body(cx: &mut TestAppContext, path: &PathBuf) -> String {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    let text = fs::read_to_string(path).unwrap();
    text.split_once("---\n")
        .and_then(|(_, rest)| rest.split_once("---\n"))
        .map(|(_, body)| body.to_string())
        .unwrap_or(text)
}

#[gpui_kit::test]
fn clicking_the_header_opens_it_and_arrows_and_enter_change_the_type(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!WARNING]\n> Careful\n");
    assert!(!menu_open(cx, &f));
    click(cx, &f, None, "Warning callout");
    assert!(menu_open(cx, &f));

    // The highlight starts on Warning; Caution is next.
    press(cx, &f, "down enter");
    assert!(!menu_open(cx, &f));
    // The caret stayed at the end of the body.
    input(cx, &f, " now");
    assert_eq!(saved_body(cx, &path), "> [!CAUTION]\n> Careful now\n");
}

#[gpui_kit::test]
fn the_shortcut_opens_it_and_plain_quote_saves_a_plain_quote(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!NOTE]\n> Body\n");
    press(cx, &f, SHORTCUT);
    assert!(menu_open(cx, &f));
    // Up from Note wraps to the last entry, "Plain quote".
    press(cx, &f, "up tab");
    assert!(!menu_open(cx, &f));
    input(cx, &f, "!");
    assert_eq!(saved_body(cx, &path), "> Body!\n");
}

#[gpui_kit::test]
fn clicking_an_entry_picks_it(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!TIP]\n> Body\n");
    click(cx, &f, None, "Tip callout");
    click(cx, &f, Some("block-editor-callout-menu"), "Important");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "> [!IMPORTANT]\n> Body\n");
}

#[gpui_kit::test]
fn the_shortcut_outside_a_callout_does_nothing(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!NOTE]\n> Body\n\nAfter\n");
    let before = fs::read(&path).unwrap();
    press(cx, &f, SHORTCUT);
    assert!(!menu_open(cx, &f));
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn the_shortcut_does_not_open_it_while_the_header_is_scrolled_away(cx: &mut TestAppContext) {
    // The caret ends up on the last line, far below the header.
    let body: String = (0..120).map(|n| format!(">\n> line {n}\n")).collect();
    let (f, path) = open_note(cx, &format!("> [!NOTE]\n> first\n{body}"));
    let before = fs::read(&path).unwrap();
    press(cx, &f, SHORTCUT);
    assert!(!menu_open(cx, &f));
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn one_undo_restores_the_old_type_and_the_caret_stays(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!WARNING]\n> Careful\n");
    press(cx, &f, SHORTCUT);
    press(cx, &f, "down enter");
    press(cx, &f, "secondary-z");
    input(cx, &f, "!");
    assert_eq!(saved_body(cx, &path), "> [!WARNING]\n> Careful!\n");
}

/// Opens the Personal Inbox's Board with one Task whose body is a callout,
/// opens the Task, and puts the caret at the end of its body.
fn open_task(cx: &mut TestAppContext) -> (Fixture, PathBuf) {
    let f = open_shell(cx);
    let tasks = f.data_root.join("personal/projects/inbox/tasks");
    fs::create_dir_all(&tasks).unwrap();
    let path = tasks.join("a.md");
    fs::write(
        &path,
        "---\ntitle: a\ncolumn: Todo\norder: 1\n---\n> [!NOTE]\n> Body\n",
    )
    .unwrap();
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(Change::Project("inbox".into()), cx)
        })
    });
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("card-a", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
    })
    .unwrap();
    cx.run_until_parked();
    (f, path)
}

fn showing_task(cx: &mut TestAppContext, f: &Fixture) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find("task-view").is_some()
    })
    .unwrap()
}

#[gpui_kit::test]
fn escape_closes_the_menu_but_not_the_task_view(cx: &mut TestAppContext) {
    let (f, path) = open_task(cx);
    let before = fs::read(&path).unwrap();
    press(cx, &f, SHORTCUT);
    assert!(menu_open(cx, &f));
    press(cx, &f, "escape");
    assert!(!menu_open(cx, &f));
    assert!(showing_task(cx, &f));
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn clicking_outside_closes_it_without_a_pick(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!NOTE]\n> Body\n");
    press(cx, &f, SHORTCUT);
    // Down to Tip: a click outside must not pick it.
    press(cx, &f, "down");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!menu_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "> [!NOTE]\n> Body\n");
}

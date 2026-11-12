//! UI integration tests for the Code Block Language Menu: opening it from
//! the label or the keyboard, filtering by typing, picking a language, and
//! what each pick writes to the file.

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

const SHORTCUT: &str = if cfg!(target_os = "macos") {
    "cmd-alt-l"
} else {
    "ctrl-alt-l"
};

const MENU: &str = "block-editor-language-menu";

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

fn has(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label(label).is_some()
    })
    .unwrap()
}

fn menu_open(cx: &mut TestAppContext, f: &Fixture) -> bool {
    has(cx, f, "Language menu")
}

fn saved(cx: &mut TestAppContext, path: &PathBuf) -> String {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    let text = fs::read_to_string(path).unwrap();
    text.split_once("---\n")
        .and_then(|(_, rest)| rest.split_once("---\n"))
        .map(|(_, body)| body.to_string())
        .unwrap_or(text)
}

#[gpui_kit::test]
fn a_click_on_the_label_and_the_shortcut_each_open_it(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, "```rust\nlet a = 1;\n```\n");
    assert!(!menu_open(cx, &f));
    click(cx, &f, Some("note-body"), "Language: Rust");
    assert!(menu_open(cx, &f));
    press(cx, &f, "escape");
    assert!(!menu_open(cx, &f));
    press(cx, &f, SHORTCUT);
    assert!(menu_open(cx, &f));
}

#[gpui_kit::test]
fn the_shortcut_outside_a_code_block_does_nothing(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust\nlet a = 1;\n```\n\nAfter\n");
    let before = fs::read(&path).unwrap();
    press(cx, &f, SHORTCUT);
    assert!(!menu_open(cx, &f));
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn the_shortcut_opens_nothing_while_the_label_is_scrolled_away(cx: &mut TestAppContext) {
    let lines: String = (0..150).map(|n| format!("line {n}\n")).collect();
    let (f, path) = open_note(cx, &format!("```rust\n{lines}```\n"));
    let before = fs::read(&path).unwrap();
    press(cx, &f, SHORTCUT);
    assert!(!menu_open(cx, &f));
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[gpui_kit::test]
fn typing_filters_the_rows_and_never_reaches_the_code(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust\nlet a = 1;\n```\n");
    press(cx, &f, SHORTCUT);
    assert!(has(cx, &f, "Plain text"));
    input(cx, &f, "ty");
    assert!(has(cx, &f, "TypeScript"));
    assert!(!has(cx, &f, "Rust"));
    assert!(!has(cx, &f, "Plain text"));
    press(cx, &f, "escape");
    assert_eq!(saved(cx, &path), "```rust\nlet a = 1;\n```\n");
}

#[gpui_kit::test]
fn an_alias_finds_its_language_and_backspace_edits_the_query(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, "```\nx\n```\n");
    press(cx, &f, SHORTCUT);
    input(cx, &f, "rs");
    assert!(has(cx, &f, "Rust"));
    assert!(!has(cx, &f, "TypeScript"));
    press(cx, &f, "backspace backspace");
    assert!(has(cx, &f, "TypeScript"));
}

#[gpui_kit::test]
fn the_current_language_is_ticked_through_an_alias(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, "```RS\nx\n```\n");
    press(cx, &f, SHORTCUT);
    assert!(has(cx, &f, "Rust, current"));
    assert!(!has(cx, &f, "Python, current"));
}

#[gpui_kit::test]
fn picking_rust_writes_the_fence_name(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```\nx\n```\n");
    press(cx, &f, SHORTCUT);
    input(cx, &f, "rust");
    press(cx, &f, "enter");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved(cx, &path), "```rust\nx\n```\n");
}

#[gpui_kit::test]
fn a_click_on_a_row_picks_it(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```\nx\n```\n");
    click(cx, &f, Some("note-body"), "Language: Plain text");
    // Filtered, so the row is inside the test window.
    input(cx, &f, "py");
    click(cx, &f, Some(MENU), "Python");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved(cx, &path), "```python\nx\n```\n");
}

#[gpui_kit::test]
fn use_writes_the_query_as_typed_and_hides_for_whitespace(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```\nx\n```\n");
    press(cx, &f, SHORTCUT);
    assert!(!has(cx, &f, "Use \"\""));
    input(cx, &f, "a b");
    assert!(!has(cx, &f, "Use \"a b\""));
    press(cx, &f, "backspace backspace backspace");
    input(cx, &f, "haskell");
    assert!(has(cx, &f, "Use \"haskell\""));
    press(cx, &f, "enter");
    assert_eq!(saved(cx, &path), "```haskell\nx\n```\n");
}

#[gpui_kit::test]
fn use_is_hidden_when_a_row_already_has_that_name(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, "```\nx\n```\n");
    press(cx, &f, SHORTCUT);
    input(cx, &f, "rust");
    assert!(!has(cx, &f, "Use \"rust\""));
}

#[gpui_kit::test]
fn plain_text_drops_the_language_and_its_meta(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust title=x\nx\n```\n");
    press(cx, &f, SHORTCUT);
    // Plain text is the first row.
    click(cx, &f, Some(MENU), "Plain text");
    assert_eq!(saved(cx, &path), "```\nx\n```\n");
}

#[gpui_kit::test]
fn changing_the_language_keeps_the_meta(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust title=x\nx\n```\n");
    press(cx, &f, SHORTCUT);
    input(cx, &f, "py");
    click(cx, &f, Some(MENU), "Python");
    assert_eq!(saved(cx, &path), "```python title=x\nx\n```\n");
}

#[gpui_kit::test]
fn one_undo_restores_the_old_language_and_the_caret_stays(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust\nx\n```\n");
    press(cx, &f, SHORTCUT);
    input(cx, &f, "python");
    press(cx, &f, "enter");
    press(cx, &f, "secondary-z");
    input(cx, &f, "!");
    assert_eq!(saved(cx, &path), "```rust\nx!\n```\n");
}

#[gpui_kit::test]
fn escape_changes_nothing(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust\nx\n```\n");
    press(cx, &f, SHORTCUT);
    press(cx, &f, "up down escape");
    assert!(!menu_open(cx, &f));
    input(cx, &f, "!");
    assert_eq!(saved(cx, &path), "```rust\nx!\n```\n");
}

#[gpui_kit::test]
fn clicking_outside_closes_it_without_a_pick(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```rust\nx\n```\n");
    press(cx, &f, SHORTCUT);
    press(cx, &f, "down");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!menu_open(cx, &f));
    assert_eq!(saved(cx, &path), "```rust\nx\n```\n");
}

#[gpui_kit::test]
fn the_highlight_starts_on_the_current_language_and_wraps(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "```\nx\n```\n");
    // A bare fence starts on Plain text; up wraps to the last row.
    press(cx, &f, SHORTCUT);
    press(cx, &f, "up enter");
    assert_eq!(saved(cx, &path), "```zig\nx\n```\n");
    // Now Zig is current, so down wraps to Plain text.
    press(cx, &f, SHORTCUT);
    press(cx, &f, "down enter");
    assert_eq!(saved(cx, &path), "```\nx\n```\n");
}

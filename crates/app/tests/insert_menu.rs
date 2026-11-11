//! UI integration tests for the Insert Menu: typing `/` at the start of an
//! empty block in a Note's body, filtering, picking with keys or the mouse,
//! closing without losing text, and undo.

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

/// Opens a Note whose body is one line, with the caret on a new empty line
/// under it.
fn on_a_new_line(cx: &mut TestAppContext) -> (Fixture, PathBuf) {
    let f = open_shell(cx);
    let path = f.data_root.join("personal/notes/plan.md");
    fs::write(&path, "---\ntitle: Plan\n---\nFirst line\n").unwrap();
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
        window.press("enter", cx);
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

fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label(label).is_some()
    })
    .unwrap()
}

fn menu_open(cx: &mut TestAppContext, f: &Fixture) -> bool {
    has_label(cx, f, "Insert Menu")
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
fn typing_a_query_filters_and_enter_picks_the_match(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/");
    assert!(menu_open(cx, &f));
    assert!(has_label(cx, &f, "Heading 1") && has_label(cx, &f, "Divider"));

    input(cx, &f, "che");
    assert!(has_label(cx, &f, "Checklist"));
    assert!(!has_label(cx, &f, "Heading 1"));

    press(cx, &f, "enter");
    assert!(!menu_open(cx, &f));
    input(cx, &f, "milk");
    assert_eq!(saved_body(cx, &path), "First line\n\n- [ ] milk\n");
}

#[gpui_kit::test]
fn arrows_move_the_highlight_and_tab_picks(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/");
    // Heading 1, 2, 3: down twice, then up and down again.
    press(cx, &f, "down down up down tab");
    input(cx, &f, "Title");
    assert_eq!(saved_body(cx, &path), "First line\n\n### Title\n");
}

#[gpui_kit::test]
fn clicking_an_entry_picks_it(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, Some("block-editor-insert-menu"), "Divider", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!menu_open(cx, &f));
    input(cx, &f, "after");
    assert_eq!(saved_body(cx, &path), "First line\n\n---\n\nafter\n");
}

#[gpui_kit::test]
fn escape_closes_and_keeps_the_typed_text(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/he");
    press(cx, &f, "escape");
    assert!(!menu_open(cx, &f));
    // Enter is a new line again, not a pick.
    press(cx, &f, "enter");
    input(cx, &f, "x");
    assert_eq!(saved_body(cx, &path), "First line\n\n/he\n\nx\n");
}

#[gpui_kit::test]
fn a_query_with_no_match_or_deleting_the_slash_closes(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/zz");
    assert!(!menu_open(cx, &f));
    press(cx, &f, "backspace backspace backspace");
    input(cx, &f, "/");
    assert!(menu_open(cx, &f));
    press(cx, &f, "backspace");
    assert!(!menu_open(cx, &f));
    input(cx, &f, "plain");
    assert_eq!(saved_body(cx, &path), "First line\n\nplain\n");
}

#[gpui_kit::test]
fn a_slash_inside_text_does_not_open_it(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "src/app");
    assert!(!menu_open(cx, &f));
    press(cx, &f, "enter");
    input(cx, &f, "x/");
    assert!(!menu_open(cx, &f));
}

#[gpui_kit::test]
fn one_undo_reverts_a_pick_to_the_typed_text(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/quo");
    press(cx, &f, "enter");
    press(cx, &f, "secondary-z");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n/quo\n");
}

#[gpui_kit::test]
fn callout_lists_the_five_callout_types(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "/callout");
    for label in ["Note", "Tip", "Important", "Warning", "Caution"] {
        assert!(has_label(cx, &f, label), "{label}");
    }
    assert!(!has_label(cx, &f, "Quote"));
}

#[gpui_kit::test]
fn callouts_are_not_offered_inside_a_list_item(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "- /");
    assert!(menu_open(cx, &f));
    assert!(has_label(cx, &f, "Heading 1"));
    assert!(!has_label(cx, &f, "Note"));
    input(cx, &f, "warn");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n- /warn\n");
}

#[gpui_kit::test]
fn picking_a_callout_leaves_the_caret_inside_it(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/warn");
    press(cx, &f, "enter");
    assert!(!menu_open(cx, &f));
    assert!(has_label(cx, &f, "Warning callout"));
    input(cx, &f, "Careful");
    assert_eq!(
        saved_body(cx, &path),
        "First line\n\n> [!WARNING]\n> Careful\n"
    );
}

#[gpui_kit::test]
fn one_undo_reverts_a_callout_pick_to_the_typed_text(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "/warn");
    press(cx, &f, "enter");
    press(cx, &f, "secondary-z");
    assert!(!menu_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n/warn\n");
}

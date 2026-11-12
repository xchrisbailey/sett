//! UI integration tests for the Code Block's corner language label: what it
//! reads for each kind of language, that clicking it leaves the caret alone,
//! and that none of it reaches the file.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use common::{Fixture, click_label, open_shell};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::Change;

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

const BODY: &str = "```rs\nlet b = 2;\n```\n\n```\nplain\n```\n\n```foo\nodd\n```\n\nend";

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

fn has(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    common::body_has_label(cx, f, label)
}

#[gpui_kit::test]
fn the_label_reads_the_display_name_plain_text_or_the_unknown_name(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, BODY);
    // `rs` reads "Rust".
    assert!(has(cx, &f, "Language: Rust"));
    assert!(has(cx, &f, "Language: Plain text"));
    assert!(has(cx, &f, "Language: foo"));
    assert!(!has(cx, &f, "Language: rs"));
}

#[gpui_kit::test]
fn the_full_language_name_reads_the_same(cx: &mut TestAppContext) {
    let (f, _) = open_note(cx, "```rust\nlet a = 1;\n```\n");
    assert!(has(cx, &f, "Language: Rust"));
}

#[gpui_kit::test]
fn clicking_the_label_leaves_the_caret_and_the_file_alone(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, BODY);
    let before = fs::read(&path).unwrap();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, Some("note-body"), "Language: foo", cx);
    })
    .unwrap();
    cx.run_until_parked();
    // The caret is still at the end of the body, so this lands after "end".
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input("!", cx);
    })
    .unwrap();
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    let after = fs::read_to_string(&path).unwrap();
    assert_eq!(
        after,
        String::from_utf8(before).unwrap().replace("end", "end!")
    );
}

#[gpui_kit::test]
fn opening_a_note_with_labels_saves_nothing(cx: &mut TestAppContext) {
    let (_f, path) = open_note(cx, BODY);
    let before = fs::read(&path).unwrap();
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    assert_eq!(fs::read(&path).unwrap(), before);
}

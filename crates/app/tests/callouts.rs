//! UI integration tests for drawing Callouts in a Note's body: the header,
//! clicking and typing in the body, and the empty callout's line.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use common::{Fixture, find_label, open_shell};
use gpui_kit::test::{TestEventExt as _, TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, MouseButton, ParentElement as _, Pixels,
    Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use markdown_editor::{BlockEditor, BlockEditorState, BlockKind};
use sett::Selection;
use sett_store::Change;

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

/// Opens a Note whose body is `body`.
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
    })
    .unwrap();
    cx.run_until_parked();
    (f, path)
}

fn header_bounds(cx: &mut TestAppContext, f: &Fixture, label: &str) -> Option<Bounds<Pixels>> {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find_by_label(label)
            .map(|_| find_label(window, None, label).bounds())
    })
    .unwrap()
}

/// Clicks a little under the header and well right of it: the end of the
/// callout's first line.
fn click_under_header(cx: &mut TestAppContext, f: &Fixture, label: &str) {
    let header = header_bounds(cx, f, label).expect("callout header");
    let at = point(header.right() + px(200.), header.bottom() + px(10.));
    cx.update_window(f.window, |_, window, cx| {
        window.pointer_move(at, None, cx);
        window.pointer_down(at, MouseButton::Left, cx);
        window.pointer_up(at, MouseButton::Left, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn input(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
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
fn a_callout_shows_its_header_and_a_click_in_its_body_places_the_caret(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!WARNING]\n> Careful\n");
    assert!(header_bounds(cx, &f, "Warning callout").is_some());
    click_under_header(cx, &f, "Warning callout");
    input(cx, &f, " now");
    assert_eq!(saved_body(cx, &path), "> [!WARNING]\n> Careful now\n");
}

#[gpui_kit::test]
fn typing_into_an_empty_callout_fills_its_line(cx: &mut TestAppContext) {
    let (f, path) = open_note(cx, "> [!TIP]\n");
    click_under_header(cx, &f, "Tip callout");
    input(cx, &f, "text");
    assert_eq!(saved_body(cx, &path), "> [!TIP]\n> text\n");
}

/// A bare editor in a window, outside the app.
struct Host {
    editor: Entity<BlockEditorState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(BlockEditor::new(&self.editor))
    }
}

#[gpui_kit::test]
fn the_header_row_is_recorded_for_hit_testing(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        markdown_editor::init(cx);
    });
    let (window, host) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(800.), px(600.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    let editor = cx.new(|cx| {
                        BlockEditorState::new(window, cx)
                            .with_source("Intro\n\n> [!NOTE]\n> Body\n\n- > [!TIP]\n  > Nested\n")
                    });
                    cx.new(|_| Host { editor })
                },
            )
        })
        .unwrap();
    let header = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            // Only a top-level quote is drawn as a callout.
            assert!(window.try_find_by_label("Tip callout").is_none());
            window.find_by_label("Note callout").bounds()
        })
        .unwrap();
    let editor = cx.update(|cx| host.read(cx).editor.clone());
    cx.update(|cx| {
        let state = editor.read(cx);
        let doc = state.model().document();
        let callout = doc
            .root()
            .children()
            .find(|b| matches!(b.kind(), BlockKind::Blockquote { callout: Some(_) }))
            .unwrap()
            .id();
        assert_eq!(state.callout_header_at(header.center()), Some(callout));
        let body = point(header.center().x, header.bottom() + px(12.));
        assert_eq!(state.callout_header_at(body), None);
    });
}

//! UI integration tests for Links drawn as title chips in the block editor:
//! the title, the broken and ambiguous states, and opening into source while
//! the caret is inside.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use common::{Fixture, open_shell};
use gpui_kit::test::{TestEventExt as _, TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, IntoElement, MouseButton, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use markdown_editor::{BlockEditor, BlockEditorState};
use sett::Selection;
use sett_store::Change;

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

fn write(cx: &mut TestAppContext, f: &Fixture, rel: &str, text: &str, change: Change) {
    let path = f.data_root.join("personal").join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(change, cx)));
}

fn note(slug: &str) -> Change {
    Change::Note {
        project: None,
        slug: slug.into(),
    }
}

/// Opens the Note `plan` with `body`, next to a Note "Budget plan" (`budget`)
/// and two Items sharing the slug `twin`, with the caret at the end.
fn open_plan(cx: &mut TestAppContext, body: &str) -> (Fixture, PathBuf) {
    let f = open_shell(cx);
    write(
        cx,
        &f,
        "notes/budget.md",
        "---\ntitle: Budget plan\n---\n",
        note("budget"),
    );
    write(
        cx,
        &f,
        "notes/twin.md",
        "---\ntitle: Twin\n---\n",
        note("twin"),
    );
    write(
        cx,
        &f,
        "projects/inbox/notes/twin.md",
        "---\ntitle: Twin too\n---\n",
        Change::Project("inbox".into()),
    );
    let plan = format!("---\ntitle: Plan\n---\n{body}");
    write(cx, &f, "notes/plan.md", &plan, note("plan"));
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Notes, cx)));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let path = f.data_root.join("personal/notes/plan.md");
    (f, path)
}

fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .within("note-body")
            .try_find_by_label(label)
            .is_some()
    })
    .unwrap()
}

fn press(cx: &mut TestAppContext, f: &Fixture, key: &str, times: usize) {
    cx.update_window(f.window, |_, window, cx| {
        for _ in 0..times {
            window.press(key, cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
}

fn settle(cx: &mut TestAppContext) {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_link_to_a_note_shows_its_title(cx: &mut TestAppContext) {
    let (f, _) = open_plan(
        cx,
        "See [[budget]] and [[budget#Quotes]] and [[budget|spend]]\n",
    );
    assert!(has_label(cx, &f, "Budget plan"));
    assert!(has_label(cx, &f, "Budget plan \u{203A} Quotes"));
    assert!(has_label(cx, &f, "spend"));
}

#[gpui_kit::test]
fn a_broken_and_an_ambiguous_link_show_their_state(cx: &mut TestAppContext) {
    let (f, _) = open_plan(cx, "See [[nowhere]] and [[twin]]\n");
    assert!(has_label(cx, &f, "nowhere (not found)"));
    assert!(has_label(cx, &f, "Twin (ambiguous)"));
    assert!(!has_label(cx, &f, "Twin"));
}

#[gpui_kit::test]
fn a_broken_link_shows_its_slug_not_its_label(cx: &mut TestAppContext) {
    let (f, _) = open_plan(cx, "See [[nowhere|spend]]\n");
    assert!(has_label(cx, &f, "nowhere (not found)"));
    assert!(!has_label(cx, &f, "spend (not found)"));
}

#[gpui_kit::test]
fn a_new_title_elsewhere_redraws_the_chip(cx: &mut TestAppContext) {
    let (f, _) = open_plan(cx, "See [[budget]]\n");
    assert!(has_label(cx, &f, "Budget plan"));
    write(
        cx,
        &f,
        "notes/budget.md",
        "---\ntitle: Spending\n---\n",
        note("budget"),
    );
    cx.run_until_parked();
    assert!(has_label(cx, &f, "Spending"));
    assert!(!has_label(cx, &f, "Budget plan"));
}

#[gpui_kit::test]
fn the_caret_inside_a_link_opens_it_without_changing_the_file(cx: &mut TestAppContext) {
    let body = "See [[budget]] end\n";
    let (f, path) = open_plan(cx, body);
    settle(cx);
    let before = fs::read_to_string(&path).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert!(has_label(cx, &f, "Budget plan"));

    // The caret is after "end"; four steps left sit just after `]]`.
    press(cx, &f, "left", 4);
    assert!(has_label(cx, &f, "Budget plan"));
    // One more step is inside the Link: the chip gives way to source.
    press(cx, &f, "left", 1);
    assert!(!has_label(cx, &f, "Budget plan"));
    // Stepping out the other side closes it again.
    press(cx, &f, "right", 1);
    assert!(has_label(cx, &f, "Budget plan"));
    press(cx, &f, "left", 9);
    assert!(!has_label(cx, &f, "Budget plan"));
    press(cx, &f, "left", 1);
    assert!(has_label(cx, &f, "Budget plan"));

    settle(cx);
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);

    // The moves left no undo step: the first undo removes what is typed next.
    press(cx, &f, "secondary-down", 1);
    cx.update_window(f.window, |_, window, cx| window.input("!", cx))
        .unwrap();
    press(cx, &f, "secondary-z", 1);
    settle(cx);
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
}

#[gpui_kit::test]
fn typing_inside_a_link_edits_its_source(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See [[budget]]\n");
    // Left of `]]`, inside the Link.
    press(cx, &f, "left", 2);
    cx.update_window(f.window, |_, window, cx| window.input("x", cx))
        .unwrap();
    settle(cx);
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .ends_with("See [[budgetx]]\n")
    );
    // `budgetx` has no Item.
    press(cx, &f, "right", 2);
    assert!(has_label(cx, &f, "budgetx (not found)"));
}

/// A bare editor in a window, outside the app, so a test can read its model.
struct Host {
    editor: Entity<BlockEditorState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(BlockEditor::new(&self.editor))
    }
}

const LINE: &str = "ab [[budget]] cd";

/// Clicks `dx` pixels from the left edge of the chip in `LINE` (a fraction of
/// its width when `of_width`), types "X" and returns the source.
fn click_then_type(cx: &mut TestAppContext, dx: f32, of_width: bool) -> String {
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
                    let editor = cx.new(|cx| BlockEditorState::new(window, cx).with_source(LINE));
                    cx.new(|_| Host { editor })
                },
            )
        })
        .unwrap();
    let editor = cx.update(|cx| host.read(cx).editor.clone());
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let chip = window.find_by_label("budget (not found)").bounds();
        let dx = if of_width {
            chip.size.width * dx
        } else {
            px(dx)
        };
        let at = point(chip.left() + dx, chip.center().y);
        window.pointer_move(at, None, cx);
        window.pointer_down(at, MouseButton::Left, cx);
        window.pointer_up(at, MouseButton::Left, cx);
        window.input("X", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| editor.read(cx).source())
}

/// Presses at `from` and drags to `to` (fractions of the chip's width from its
/// left edge), types "X" and returns the source.
fn drag_then_type(cx: &mut TestAppContext, from: f32, to: f32) -> String {
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
                    let editor = cx.new(|cx| BlockEditorState::new(window, cx).with_source(LINE));
                    cx.new(|_| Host { editor })
                },
            )
        })
        .unwrap();
    let editor = cx.update(|cx| host.read(cx).editor.clone());
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let chip = window.find_by_label("budget (not found)").bounds();
        let at = |f: f32| point(chip.left() + chip.size.width * f, chip.center().y);
        window.pointer_move(at(from), None, cx);
        window.pointer_down(at(from), MouseButton::Left, cx);
        window.pointer_move(at(to), Some(MouseButton::Left), cx);
        window.pointer_up(at(to), MouseButton::Left, cx);
        window.input("X", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| editor.read(cx).source())
}

#[gpui_kit::test]
fn clicks_beside_a_chip_land_at_the_right_offsets(cx: &mut TestAppContext) {
    // Just right of the chip: after `]]`, before " cd".
    assert_eq!(click_then_type(cx, 1.0, true), "ab [[budget]]X cd");
}

#[gpui_kit::test]
fn clicks_left_of_a_chip_land_before_it(cx: &mut TestAppContext) {
    assert_eq!(click_then_type(cx, -1., false), "ab X[[budget]] cd");
}

#[gpui_kit::test]
fn a_click_on_a_chip_opens_it_with_the_caret_before_the_closing_brackets(cx: &mut TestAppContext) {
    assert_eq!(click_then_type(cx, 0.25, true), "ab [[budgetX]] cd");
    assert_eq!(click_then_type(cx, 0.75, true), "ab [[budgetX]] cd");
}

fn saved(cx: &mut TestAppContext, path: &PathBuf) -> String {
    settle(cx);
    fs::read_to_string(path).unwrap()
}

#[gpui_kit::test]
fn backspace_after_a_chip_removes_the_whole_link_and_one_undo_restores_it(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See [[budget]]\n");
    let before = saved(cx, &path);
    press(cx, &f, "backspace", 1);
    assert_eq!(saved(cx, &path), "---\ntitle: Plan\n---\nSee \n");
    press(cx, &f, "secondary-z", 1);
    assert_eq!(saved(cx, &path), before);
}

#[gpui_kit::test]
fn delete_before_a_chip_removes_the_whole_link_and_one_undo_restores_it(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See [[budget]]\n");
    let before = saved(cx, &path);
    // Ten steps left: just before `[[`, outside the chip.
    press(cx, &f, "left", 10);
    press(cx, &f, "delete", 1);
    assert_eq!(saved(cx, &path), "---\ntitle: Plan\n---\nSee \n");
    press(cx, &f, "secondary-z", 1);
    assert_eq!(saved(cx, &path), before);
}

#[gpui_kit::test]
fn backspace_inside_an_open_chip_removes_one_character(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See [[budget]]\n");
    press(cx, &f, "left", 2);
    press(cx, &f, "backspace", 1);
    assert!(saved(cx, &path).ends_with("See [[budge]]\n"));
}

#[gpui_kit::test]
fn copying_across_a_chip_and_pasting_keeps_the_link_text(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See [[budget]] end\n");
    press(cx, &f, "shift-left", 14);
    press(cx, &f, "secondary-c", 1);
    press(cx, &f, "secondary-down", 1);
    press(cx, &f, "secondary-v", 1);
    assert!(saved(cx, &path).ends_with("See [[budget]] end[[budget]] end\n"));
}

#[gpui_kit::test]
fn typing_a_link_by_hand_ends_with_a_closed_chip(cx: &mut TestAppContext) {
    let (f, path) = open_plan(cx, "See\n");
    cx.update_window(f.window, |_, window, cx| {
        for c in " [[budget]]".chars() {
            window.input(&c.to_string(), cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
    let text = saved(cx, &path);
    assert!(text.ends_with("See [[budget]]\n"), "{text:?}");
    assert!(has_label(cx, &f, "Budget plan"));
}

#[gpui_kit::test]
fn a_drag_from_a_chip_starts_at_its_edge_and_does_not_open_it(cx: &mut TestAppContext) {
    // From the near half out to the end of the line.
    assert_eq!(drag_then_type(cx, 0.25, 8.0), "ab X");
    // From the far half back to the start of the line.
    assert_eq!(drag_then_type(cx, 0.75, -8.0), "X cd");
}

#[gpui_kit::test]
fn a_drag_ending_on_a_chip_stops_at_its_edge(cx: &mut TestAppContext) {
    assert_eq!(drag_then_type(cx, 8.0, 0.25), "ab X");
    assert_eq!(drag_then_type(cx, 8.0, 0.75), "ab [[budget]]X");
}

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
fn a_click_on_a_chip_snaps_to_its_nearer_edge(cx: &mut TestAppContext) {
    assert_eq!(click_then_type(cx, 0.25, true), "ab X[[budget]] cd");
}

#[gpui_kit::test]
fn a_click_on_the_far_half_of_a_chip_snaps_after_it(cx: &mut TestAppContext) {
    assert_eq!(click_then_type(cx, 0.75, true), "ab [[budget]]X cd");
}

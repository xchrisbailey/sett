//! UI integration tests for Back (Cmd-[) and Forward (Cmd-]) through the
//! Items opened in a Space.

mod common;

use std::fs;

use common::{Fixture, center_title, input, note, open_shell, press, write};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::{Change, LinkTarget};

const BACK: &str = "cmd-[";
const FORWARD: &str = "cmd-]";

/// Three Notes at Space level and a Task, with the Notes list showing.
fn setup(cx: &mut TestAppContext) -> Fixture {
    let f = open_shell(cx);
    for (slug, title) in [("alpha", "Alpha"), ("beta", "Beta"), ("gamma", "Gamma")] {
        write(
            cx,
            &f,
            &format!("notes/{slug}.md"),
            &format!("---\ntitle: {title}\n---\ntext\n"),
            note(None, slug),
        );
    }
    write(
        cx,
        &f,
        "projects/inbox/tasks/fix-bug.md",
        "---\ntitle: Fix bug\ncolumn: Todo\n---\nBody\n",
        Change::Project("inbox".into()),
    );
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |m, cx| m.select(Selection::Notes, cx)));
    cx.update_window(f.window, |_, window, cx| window.render_frame(cx))
        .unwrap();
    f
}

/// Opens a Note by clicking its row in the Notes list.
fn open_note(cx: &mut TestAppContext, f: &Fixture, slug: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("note-{slug}"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn title(cx: &mut TestAppContext, f: &Fixture) -> String {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find("note-title")
            .and_then(|n| n.value().map(str::to_string))
            .unwrap_or_default()
    })
    .unwrap()
}

/// Opens an Item the way following a Link does.
fn follow(cx: &mut TestAppContext, f: &Fixture, slug: &str) {
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |m, cx| {
            let LinkTarget::Found { item, .. } = m.index().resolve(slug) else {
                panic!("no {slug}");
            };
            m.open_item(item, cx)
        })
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn back_and_forward_walk_through_the_notes_list_opens(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "gamma");
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
    press(cx, &f, FORWARD);
    assert_eq!(title(cx, &f), "Gamma");
}

#[gpui_kit::test]
fn back_returns_to_the_item_a_link_was_followed_from(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    follow(cx, &f, "beta");
    assert_eq!(title(cx, &f), "Beta");
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn at_either_end_the_actions_do_nothing(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
    press(cx, &f, FORWARD);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn reopening_the_current_item_adds_no_entry(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "beta");
    follow(cx, &f, "beta");
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn a_new_open_after_going_back_drops_the_forward_entries(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "beta");
    press(cx, &f, BACK);
    open_note(cx, &f, "gamma");
    press(cx, &f, FORWARD);
    assert_eq!(title(cx, &f), "Gamma");
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn items_that_no_longer_exist_are_skipped(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "beta");
    open_note(cx, &f, "gamma");
    fs::remove_file(f.data_root.join("personal/notes/beta.md")).unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |m, cx| m.apply_change(note(None, "beta"), cx)));
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
    press(cx, &f, FORWARD);
    assert_eq!(title(cx, &f), "Gamma");
}

#[gpui_kit::test]
fn going_back_to_a_task_opens_it_over_its_board(cx: &mut TestAppContext) {
    let f = setup(cx);
    follow(cx, &f, "fix-bug");
    follow(cx, &f, "alpha");
    press(cx, &f, BACK);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("task-title").value(), Some("Fix bug"));
    })
    .unwrap();
    press(cx, &f, FORWARD);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn going_back_saves_pending_edits_first(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "beta");
    cx.update_window(f.window, |_, window, cx| {
        window.click("note-body", cx);
        window.press("secondary-down", cx);
    })
    .unwrap();
    input(cx, &f, "Z");
    // No autosave yet: leaving the Item must write it.
    press(cx, &f, BACK);
    let text = fs::read_to_string(f.data_root.join("personal/notes/beta.md")).unwrap();
    assert!(text.contains("textZ"), "{text}");
}

#[gpui_kit::test]
fn each_space_keeps_its_own_history(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    open_note(cx, &f, "beta");
    cx.update_window(f.window, |_, window, cx| window.press("cmd-2", cx))
        .unwrap();
    cx.run_until_parked();
    // Nothing was opened in Work, so Back has nowhere to go there.
    press(cx, &f, BACK);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(center_title(window), "Inbox");
    })
    .unwrap();
    cx.update_window(f.window, |_, window, cx| window.press("cmd-1", cx))
        .unwrap();
    cx.run_until_parked();
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
}

#[gpui_kit::test]
fn the_journal_is_part_of_the_history(cx: &mut TestAppContext) {
    let f = setup(cx);
    open_note(cx, &f, "alpha");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("nav").click("0-0", cx);
    })
    .unwrap();
    cx.run_until_parked();
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
    press(cx, &f, FORWARD);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(center_title(window), "Journal");
    })
    .unwrap();
}

#[gpui_kit::test]
fn back_from_the_board_reopens_the_task_just_closed(cx: &mut TestAppContext) {
    let f = setup(cx);
    follow(cx, &f, "alpha");
    follow(cx, &f, "fix-bug");
    // Escape is the Task view's "Back to Board".
    press(cx, &f, "escape");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("task-title").is_none());
    })
    .unwrap();

    press(cx, &f, BACK);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("task-title").value(), Some("Fix bug"));
    })
    .unwrap();
    press(cx, &f, BACK);
    assert_eq!(title(cx, &f), "Alpha");
}

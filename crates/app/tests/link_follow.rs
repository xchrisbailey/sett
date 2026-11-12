//! UI integration tests for Cmd/Ctrl-clicking a Link chip: it opens the
//! Item, optionally at a heading, and a broken Link does nothing.

mod common;

use std::fs;
use std::time::Duration;

use common::{Fixture, center_title, open_shell};
use gpui_kit::test::{TestEventExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext as _, InputEvent as _, Modifiers, ModifiersChangedEvent, MouseButton,
    TestAppContext, Window,
};
use sett::Selection;
use sett_store::{Change, Date};

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

/// Opens the Note `plan` with `body`, next to a Note "Budget plan" with
/// headings, a Task "Fix bug" and a Journal Entry.
fn open_plan(cx: &mut TestAppContext, body: &str) -> Fixture {
    let f = open_shell(cx);
    write(
        cx,
        &f,
        "notes/budget.md",
        "---\ntitle: Budget plan\n---\nIntro\n\n# Overview\n\ntext\n\n## Quotes\n\nmore\n",
        note("budget"),
    );
    write(
        cx,
        &f,
        "projects/inbox/tasks/fix-bug.md",
        "---\ntitle: Fix bug\ncolumn: Todo\n---\nBody\n",
        Change::Project("inbox".into()),
    );
    write(
        cx,
        &f,
        "journal/2026-11-02.md",
        "Talked about stuff\n",
        Change::Journal(Date::parse("2026-11-02").unwrap()),
    );
    write(
        cx,
        &f,
        "notes/plan.md",
        &format!("---\ntitle: Plan\n---\n{body}"),
        note("plan"),
    );
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Notes, cx)));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    f
}

fn with_cmd(window: &mut Window, held: bool, cx: &mut gpui_kit::App) {
    let modifiers = if held {
        Modifiers {
            platform: cfg!(target_os = "macos"),
            control: cfg!(not(target_os = "macos")),
            ..Modifiers::none()
        }
    } else {
        Modifiers::none()
    };
    window.dispatch_event(
        ModifiersChangedEvent {
            modifiers,
            capslock: window.capslock(),
        }
        .to_platform_input(),
        cx,
    );
}

/// Clicks the chip with this label, with Cmd/Ctrl held or not.
fn click_chip(cx: &mut TestAppContext, f: &Fixture, label: &str, cmd: bool) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let at = window
            .within("note-body")
            .find_by_label(label)
            .bounds()
            .center();
        with_cmd(window, cmd, cx);
        window.pointer_move(at, None, cx);
        window.pointer_down(at, MouseButton::Left, cx);
        window.pointer_up(at, MouseButton::Left, cx);
        with_cmd(window, false, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn type_text(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| window.input(text, cx))
        .unwrap();
    cx.run_until_parked();
}

fn settle(cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_secs(2));
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

#[gpui_kit::test]
fn cmd_click_on_a_note_chip_opens_that_note(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget]]\n");
    click_chip(cx, &f, "Budget plan", true);
    assert_eq!(title(cx, &f), "Budget plan");
}

#[gpui_kit::test]
fn a_plain_click_on_a_chip_does_not_navigate(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget]]\n");
    click_chip(cx, &f, "Budget plan", false);
    assert_eq!(title(cx, &f), "Plan");
}

#[gpui_kit::test]
fn cmd_click_on_a_task_chip_opens_it_over_its_board(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[fix-bug]]\n");
    click_chip(cx, &f, "Fix bug", true);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("task-title").value(), Some("Fix bug"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn cmd_click_on_a_journal_link_opens_that_entry(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[2026-11-02]]\n");
    click_chip(cx, &f, "2026-11-02", true);
    let model = f.model(cx);
    assert_eq!(
        cx.update(|cx| model.read(cx).selection()),
        Selection::Journal
    );
    cx.update_window(f.window, |_, window, _| {
        assert_eq!(center_title(window), "Journal")
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_heading_link_opens_with_the_caret_in_that_heading(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget#quotes|jump]]\n");
    click_chip(cx, &f, "jump", true);
    assert_eq!(title(cx, &f), "Budget plan");
    type_text(cx, &f, "X");
    settle(cx);
    let text = fs::read_to_string(f.data_root.join("personal/notes/budget.md")).unwrap();
    assert!(text.contains("## XQuotes\n"), "{text}");
}

#[gpui_kit::test]
fn a_missing_heading_opens_the_item_normally(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget#Nowhere|jump]]\n");
    click_chip(cx, &f, "jump", true);
    assert_eq!(title(cx, &f), "Budget plan");
}

#[gpui_kit::test]
fn a_broken_link_does_nothing(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[nowhere]]\n");
    click_chip(cx, &f, "nowhere (not found)", true);
    assert_eq!(title(cx, &f), "Plan");
}

#[gpui_kit::test]
fn holding_cmd_over_chips_changes_nothing_in_the_file(cx: &mut TestAppContext) {
    // The look and the tooltip are drawn by GPUI and can't be read back
    // here; this checks that hovering with Cmd going down and up over a
    // working and a broken chip leaves the Item alone.
    let f = open_plan(cx, "See [[budget]] and [[nowhere]]\n");
    let path = f.data_root.join("personal/notes/plan.md");
    let before = fs::read_to_string(&path).unwrap();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        for label in ["Budget plan", "nowhere (not found)"] {
            let at = window
                .within("note-body")
                .find_by_label(label)
                .bounds()
                .center();
            with_cmd(window, true, cx);
            window.pointer_move(at, None, cx);
            window.render_frame(cx);
            with_cmd(window, false, cx);
            window.render_frame(cx);
        }
    })
    .unwrap();
    settle(cx);
    assert_eq!(title(cx, &f), "Plan");
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
}

#[gpui_kit::test]
fn pending_edits_in_the_item_you_leave_are_saved(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget]]\n");
    cx.update_window(f.window, |_, window, cx| {
        window.click("note-body", cx);
        window.press("secondary-down", cx);
    })
    .unwrap();
    type_text(cx, &f, "Z");
    // No autosave yet: the click itself must write it.
    click_chip(cx, &f, "Budget plan", true);
    let text = fs::read_to_string(f.data_root.join("personal/notes/plan.md")).unwrap();
    assert!(text.contains("See [[budget]]Z"), "{text}");
}

#[gpui_kit::test]
fn a_self_link_with_a_heading_reveals_it_without_reopening(cx: &mut TestAppContext) {
    let f = open_plan(cx, "# Top\n\ntext\n\n## Later\n\nSee [[plan#later|here]]\n");
    click_chip(cx, &f, "here", true);
    assert_eq!(title(cx, &f), "Plan");
    type_text(cx, &f, "X");
    settle(cx);
    let text = fs::read_to_string(f.data_root.join("personal/notes/plan.md")).unwrap();
    assert!(text.contains("## XLater\n"), "{text}");
}

#[gpui_kit::test]
fn a_self_link_without_a_heading_does_nothing(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[plan]]\n");
    click_chip(cx, &f, "Plan", true);
    assert_eq!(title(cx, &f), "Plan");
}

fn press_key(cx: &mut TestAppContext, f: &Fixture, keys: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.press(keys, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_keyboard_follows_the_link_the_caret_touches(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget]]\n");
    cx.update_window(f.window, |_, window, cx| window.click("note-body", cx))
        .unwrap();
    // The caret goes to the end of the document, right after the chip.
    press_key(cx, &f, "secondary-down");
    press_key(cx, &f, "secondary-alt-enter");
    assert_eq!(title(cx, &f), "Budget plan");
}

#[gpui_kit::test]
fn the_keyboard_route_follows_a_heading_link(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget#quotes|jump]]\n");
    cx.update_window(f.window, |_, window, cx| window.click("note-body", cx))
        .unwrap();
    press_key(cx, &f, "secondary-down");
    press_key(cx, &f, "secondary-alt-enter");
    assert_eq!(title(cx, &f), "Budget plan");
    type_text(cx, &f, "X");
    settle(cx);
    let text = fs::read_to_string(f.data_root.join("personal/notes/budget.md")).unwrap();
    assert!(text.contains("## XQuotes\n"), "{text}");
}

#[gpui_kit::test]
fn the_keyboard_route_does_nothing_away_from_a_link(cx: &mut TestAppContext) {
    let f = open_plan(cx, "See [[budget]] and more\n");
    cx.update_window(f.window, |_, window, cx| window.click("note-body", cx))
        .unwrap();
    press_key(cx, &f, "secondary-down");
    press_key(cx, &f, "secondary-alt-enter");
    assert_eq!(title(cx, &f), "Plan");
}

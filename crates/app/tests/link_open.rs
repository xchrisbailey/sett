//! UI integration tests for Cmd/Ctrl-clicking a web link in an Item's body:
//! it opens the default browser, and other schemes are ignored.

mod common;

use common::{Fixture, note, open_shell, write};
use gpui_kit::test::{TestEventExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext as _, InputEvent as _, Modifiers, ModifiersChangedEvent, MouseButton,
    TestAppContext, Window,
};
use sett::Selection;

fn open_plan(cx: &mut TestAppContext, body: &str) -> Fixture {
    let f = open_shell(cx);
    write(
        cx,
        &f,
        "notes/plan.md",
        &format!("---\ntitle: Plan\n---\n{body}"),
        note(None, "plan"),
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

/// Clicks the link at the start of the body, with Cmd/Ctrl held or not.
fn click_link(cx: &mut TestAppContext, f: &Fixture, cmd: bool) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        // Link text has no accessible label, so click inside the body's
        // first line (past the body's padding), where each test puts its link.
        let body = window.find("note-body").bounds();
        let at = body.origin + gpui_kit::point(gpui_kit::px(44.0), gpui_kit::px(24.0));
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

#[gpui_kit::test]
fn cmd_click_on_a_web_link_opens_the_browser(cx: &mut TestAppContext) {
    let f = open_plan(cx, "[docs](https://example.com/docs)\n");
    click_link(cx, &f, true);
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/docs"));
}

#[gpui_kit::test]
fn a_plain_click_on_a_web_link_opens_nothing(cx: &mut TestAppContext) {
    let f = open_plan(cx, "[docs](https://example.com/docs)\n");
    click_link(cx, &f, false);
    assert_eq!(cx.opened_url(), None);
}

#[gpui_kit::test]
fn cmd_click_on_a_file_link_opens_nothing(cx: &mut TestAppContext) {
    let f = open_plan(cx, "[evil](file:///etc/passwd)\n");
    click_link(cx, &f, true);
    assert_eq!(cx.opened_url(), None);
}

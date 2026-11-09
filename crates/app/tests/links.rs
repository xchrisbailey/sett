//! UI integration tests for the Links under an Item's body: the backlinks
//! list and following a Link from it.

mod common;

use std::fs;

use common::{Fixture, center_title, click_label, open_shell};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::{Change, ItemId};

fn write(cx: &mut TestAppContext, f: &Fixture, rel: &str, text: &str, change: Change) {
    let path = f.data_root.join("personal").join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    let workspace = f.workspace(cx);
    cx.update(|cx| workspace.update(cx, |w, cx| w.apply_change(change, cx)));
}

fn note(project: Option<&str>, slug: &str) -> Change {
    Change::Note {
        project: project.map(str::to_string),
        slug: slug.to_string(),
    }
}

fn labels(cx: &mut TestAppContext, f: &Fixture, list: &'static str) -> Vec<String> {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find(list)
            .map(|_| {
                window
                    .within(list)
                    .find_all_by_role(gpui_kit::Role::Button)
                    .iter()
                    .filter_map(|n| n.label().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    })
    .unwrap()
}

fn open_note(cx: &mut TestAppContext, f: &Fixture, project: Option<&str>, slug: &str) {
    let workspace = f.workspace(cx);
    let item = ItemId::Note {
        project: project.map(str::to_string),
        slug: slug.to_string(),
    };
    cx.update(|cx| workspace.update(cx, |w, cx| w.open_item(item, cx)));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn backlinks_show_under_the_body_and_open_their_item(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    write(
        cx,
        &f,
        "notes/plan.md",
        "---\ntitle: Plan\n---\nThe plan\n",
        note(None, "plan"),
    );
    write(
        cx,
        &f,
        "notes/ideas.md",
        "---\ntitle: Ideas\n---\nSee [[plan]] and [[nowhere]]\n",
        note(None, "ideas"),
    );
    write(
        cx,
        &f,
        "journal/2026-11-02.md",
        "Talked about [[plan|the plan]]\n",
        Change::Journal(sett_store::Date::parse("2026-11-02").unwrap()),
    );
    open_note(cx, &f, None, "plan");

    assert_eq!(labels(cx, &f, "backlinks"), ["Ideas", "2026-11-02"]);
    assert!(labels(cx, &f, "outgoing-links").is_empty());

    cx.update_window(f.window, |_, window, cx| {
        click_label(window, Some("backlinks"), "Ideas", cx);
    })
    .unwrap();
    cx.run_until_parked();
    // Now showing Ideas: its own Links, with the broken one not clickable.
    assert_eq!(labels(cx, &f, "outgoing-links"), ["Plan"]);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .within("outgoing-links")
                .try_find_by_label("nowhere (not found)")
                .is_some()
        );
    })
    .unwrap();

    cx.update_window(f.window, |_, window, cx| {
        click_label(window, Some("outgoing-links"), "Plan", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, Some("backlinks"), "2026-11-02", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let workspace = f.workspace(cx);
    assert_eq!(
        cx.update(|cx| workspace.read(cx).selection()),
        Selection::Journal
    );
    cx.update_window(f.window, |_, window, _| {
        assert_eq!(center_title(window), "Journal")
    })
    .unwrap();
    assert_eq!(labels(cx, &f, "outgoing-links"), ["Plan"]);
}

#[gpui_kit::test]
fn a_backlink_from_a_project_note_opens_the_projects_notes(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    write(
        cx,
        &f,
        "notes/plan.md",
        "---\ntitle: Plan\n---\n",
        note(None, "plan"),
    );
    write(
        cx,
        &f,
        "projects/inbox/notes/spec.md",
        "---\ntitle: Spec\n---\nPer [[plan]]\n",
        Change::Project("inbox".into()),
    );
    open_note(cx, &f, None, "plan");

    cx.update_window(f.window, |_, window, cx| {
        click_label(window, Some("backlinks"), "Spec", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let workspace = f.workspace(cx);
    assert_eq!(
        cx.update(|cx| workspace.read(cx).selection()),
        Selection::Project("inbox".into())
    );
    assert_eq!(labels(cx, &f, "outgoing-links"), ["Plan"]);
}

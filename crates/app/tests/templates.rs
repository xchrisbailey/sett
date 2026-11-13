//! UI integration tests for the Templates screen: creating, renaming,
//! deleting and following changes made outside sett.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{Fixture, click_label, open_shell, settle};
use gpui_kit::test::{TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{AppContext as _, ElementId, TestAppContext};
use sett::Selection;
use sett_store::Change;

fn templates_dir(f: &Fixture) -> PathBuf {
    f.data_root.join("personal/templates")
}

fn show_templates(cx: &mut TestAppContext, f: &Fixture) {
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Templates, cx)));
    cx.run_until_parked();
}

fn template_change(slug: &str) -> Change {
    Change::Template(slug.to_string())
}

fn click(cx: &mut TestAppContext, f: &Fixture, label: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, None, label, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn listed(cx: &mut TestAppContext, f: &Fixture) -> Vec<String> {
    let list = ElementId::Name("templates-list".into());
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .find_all_by_role(gpui_kit::Role::ListItem)
            .iter()
            .filter(|n| n.path().contains(&list))
            .filter_map(|n| n.label().map(str::to_string))
            .collect()
    })
    .unwrap()
}

fn editor_open(cx: &mut TestAppContext, f: &Fixture) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find("note-body").is_some()
    })
    .unwrap()
}

#[gpui_kit::test]
fn new_template_creates_and_opens_it_with_the_title_focused(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    show_templates(cx, &f);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-template", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let path = templates_dir(&f).join("untitled-template.md");
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("title: Untitled template")
    );
    assert!(editor_open(cx, &f));
    assert_eq!(listed(cx, &f), ["Untitled template"]);

    // The title is focused and selected, so typing replaces it.
    common::input(cx, &f, "Bug");
    settle(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("title: Bug"), "{text}");
}

fn write_template(cx: &mut TestAppContext, f: &Fixture, slug: &str, title: &str) -> PathBuf {
    let path = templates_dir(f).join(format!("{slug}.md"));
    fs::create_dir_all(templates_dir(f)).unwrap();
    fs::write(&path, format!("---\ntitle: {title}\n---\nBody\n")).unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(template_change(slug), cx)));
    cx.run_until_parked();
    path
}

fn open_template(cx: &mut TestAppContext, f: &Fixture, slug: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("template-{slug}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn committing_a_new_title_renames_the_file_and_the_editor_stays_open(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    show_templates(cx, &f);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-template", cx);
    })
    .unwrap();
    cx.run_until_parked();

    common::input(cx, &f, "Bug");
    settle(cx);
    // Autosave writes the title without renaming the file.
    let old = templates_dir(&f).join("untitled-template.md");
    assert!(fs::read_to_string(&old).unwrap().contains("title: Bug"));

    common::press(cx, &f, "enter");
    let new = templates_dir(&f).join("bug.md");
    assert!(!old.exists());
    assert!(fs::read_to_string(&new).unwrap().contains("title: Bug"));
    assert!(editor_open(cx, &f));
    assert_eq!(listed(cx, &f), ["Bug"]);

    // Edits after the rename land in the renamed file.
    common::press(cx, &f, "tab");
    common::input(cx, &f, "x");
    settle(cx);
    assert!(editor_open(cx, &f));
    assert!(!old.exists());
}

#[gpui_kit::test]
fn delete_removes_a_template_after_confirmation(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let model = f.model(cx);
    // Stands in for the macOS Trash, which these tests must not touch.
    cx.update(|cx| model.update(cx, |w, _| w.set_trash(|path| fs::remove_file(path))));
    let path = write_template(cx, &f, "bug", "Bug");
    show_templates(cx, &f);
    open_template(cx, &f, "bug");

    cx.update_window(f.window, |_, window, cx| {
        window.right_click("template-bug", cx);
    })
    .unwrap();
    cx.run_until_parked();
    click(cx, &f, "Delete\u{2026}");
    click(cx, &f, "Cancel");
    assert!(path.exists());

    cx.update_window(f.window, |_, window, cx| {
        window.right_click("template-bug", cx);
    })
    .unwrap();
    cx.run_until_parked();
    click(cx, &f, "Delete\u{2026}");
    click(cx, &f, "Delete");
    settle(cx);

    assert!(!path.exists());
    assert!(!editor_open(cx, &f));
    assert!(listed(cx, &f).is_empty());
}

#[gpui_kit::test]
fn deleting_the_file_on_disk_closes_the_editor(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = write_template(cx, &f, "bug", "Bug");
    show_templates(cx, &f);
    open_template(cx, &f, "bug");
    assert!(editor_open(cx, &f));

    fs::remove_file(&path).unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(template_change("bug"), cx)));
    cx.run_until_parked();

    assert!(!editor_open(cx, &f));
    assert!(listed(cx, &f).is_empty());
}

#[gpui_kit::test]
fn a_file_added_on_disk_appears_in_the_list(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    show_templates(cx, &f);
    assert!(common::has_label(cx, &f, "New template"));
    assert!(common::has_label(
        cx,
        &f,
        "Templates are starter bodies for Tasks and Notes. Type / in a body to use one."
    ));

    write_template(cx, &f, "bug", "Bug");
    write_template(cx, &f, "adr", "ADR");
    assert_eq!(listed(cx, &f), ["ADR", "Bug"]);
}

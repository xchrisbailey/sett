//! UI integration tests for managing Projects from the nav: creating one
//! through the dialog, archiving and unarchiving, and deleting.

mod common;

use std::fs;

use common::{center_title, click_label, open_shell};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext as _, TestAppContext};

/// Nav rows by position: the Sidebar names its rows `<menu>-<row>`, and
/// submenu rows `<menu>-<row>-<child>`. Projects sort Inbox first, then by
/// slug; archived ones sit in a folded "Archived" row in the last menu.
const INBOX: &str = "1-0";
const WEBSITE: &str = "1-1";
const ARCHIVED: &str = "2-0";
const ARCHIVED_WEBSITE: &str = "2-0-0";

#[gpui_kit::test]
fn creating_a_project_through_the_dialog_adds_it_to_the_nav(cx: &mut TestAppContext) {
    let f = open_shell(cx);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-project", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        // An empty name doesn't create anything and keeps the dialog open.
        window.click("create-project", cx);
        assert!(window.try_find("project-name").is_some());

        window.click("project-name", cx);
        window.input("Website", cx);
        window.click("icon-filter", cx);
        window.input("code", cx);
        // The filter hides icons whose name doesn't match.
        assert!(window.try_find("icon-folder").is_none());
        window.click("icon-code", cx);
        window.click("create-project", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("project-name").is_none());
        assert!(window.within("nav").try_find(WEBSITE).is_some());
        // The new Project opens straight away.
        assert_eq!(center_title(window), "Website");
    })
    .unwrap();

    let project_md =
        fs::read_to_string(f.data_root.join("personal/projects/website/project.md")).unwrap();
    assert!(project_md.contains("name: Website"), "{project_md}");
    assert!(project_md.contains("icon: code"), "{project_md}");
    assert!(project_md.contains("- Todo"), "{project_md}");
}

#[gpui_kit::test]
fn archiving_moves_a_project_to_the_archived_group_and_back(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let workspace = f.workspace(cx);
    cx.update(|cx| {
        workspace.update(cx, |w, cx| w.create_project("Website", None, cx).unwrap());
    });

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("nav").right_click(WEBSITE, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Archive", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        // Archived Projects sit folded away in the last nav menu.
        assert!(window.within("nav").try_find(WEBSITE).is_none());
        window.within("nav").click(ARCHIVED, cx);
        assert!(window.within("nav").try_find(ARCHIVED_WEBSITE).is_some());
        window.within("nav").right_click(ARCHIVED_WEBSITE, cx);
    })
    .unwrap();
    cx.run_until_parked();
    let project_md = f.data_root.join("personal/projects/website/project.md");
    assert!(
        fs::read_to_string(&project_md)
            .unwrap()
            .contains("archived: true")
    );

    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Unarchive", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.within("nav").try_find(ARCHIVED).is_none());
        assert!(window.within("nav").try_find(WEBSITE).is_some());
    })
    .unwrap();
    assert!(
        fs::read_to_string(&project_md)
            .unwrap()
            .contains("archived: false")
    );
}

#[gpui_kit::test]
fn the_inbox_offers_no_archive_or_delete(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let workspace = f.workspace(cx);
    cx.update(|cx| {
        workspace.update(cx, |w, cx| w.create_project("Website", None, cx).unwrap());
    });

    // A normal Project's menu has both, so their absence below means something.
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("nav").right_click(WEBSITE, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        assert!(window.try_find_by_label("Archive").is_some());
        assert!(window.try_find_by_label("Delete…").is_some());
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.within("nav").right_click(INBOX, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, _| {
        assert!(window.try_find_by_label("Archive").is_none());
        assert!(window.try_find_by_label("Delete…").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn deleting_a_project_confirms_then_trashes_its_folder(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let workspace = f.workspace(cx);
    // Stands in for the macOS Trash, which these tests must not touch.
    cx.update(|cx| {
        workspace.update(cx, |w, cx| {
            w.set_trash(|path| fs::remove_dir_all(path));
            w.create_project("Website", None, cx).unwrap();
        });
    });
    let website = f.data_root.join("personal/projects/website");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(center_title(window), "Website");
        window.within("nav").right_click(WEBSITE, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Delete…", cx);
    })
    .unwrap();
    cx.run_until_parked();

    // Cancelling leaves the Project alone.
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(website.exists());

    cx.update_window(f.window, |_, window, cx| {
        window.within("nav").right_click(WEBSITE, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Delete…", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        click_label(window, None, "Delete", cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert!(!website.exists());
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.within("nav").try_find(WEBSITE).is_none());
        assert_eq!(center_title(window), "Inbox");
    })
    .unwrap();
}

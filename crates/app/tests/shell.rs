//! UI integration tests for the app shell: the Space toggle, the left nav and
//! the nav following changes made on disk.

use std::fs;
use std::path::{Path, PathBuf};

use gpui_kit::test::{ElementSnapshot, TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, MouseButton, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};
use sett::{Shell, Workspace};
use sett_store::{Change, Settings, Space};

struct Fixture {
    _dir: tempfile::TempDir,
    settings_path: PathBuf,
    data_root: PathBuf,
    window: AnyWindowHandle,
    shell: Entity<Shell>,
}

fn open_shell(cx: &mut TestAppContext) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.toml");
    let settings = Settings::defaults(dir.path());
    let data_root = settings.data_root.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let (window, shell) = cx.update(|cx| {
        let workspace =
            Workspace::open_unwatched(settings, Some(settings_path.clone()), cx).unwrap();
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1024.), px(720.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(workspace, window, cx)),
        )
        .unwrap()
    });
    Fixture {
        _dir: dir,
        settings_path,
        data_root,
        window,
        shell,
    }
}

/// Clicks the element with this accessible label, inside `scope` if given.
fn click_label(
    window: &mut Window,
    scope: Option<&'static str>,
    label: &str,
    cx: &mut gpui_kit::App,
) {
    let target = find_label(window, scope, label);
    let center = target.bounds().center();
    window.pointer_move(center, None, cx);
    window.pointer_down(center, MouseButton::Left, cx);
    window.pointer_up(center, MouseButton::Left, cx);
}

fn find_label(window: &mut Window, scope: Option<&'static str>, label: &str) -> ElementSnapshot {
    match scope {
        Some(scope) => window.within(scope).find_by_label(label),
        None => window.find_by_label(label),
    }
}

fn is_selected(window: &mut Window, scope: Option<&'static str>, label: &str) -> bool {
    find_label(window, scope, label).selected() == Some(true)
}

/// Nav rows by position: the Sidebar names its rows `<menu>-<row>`.
const JOURNAL: &str = "0-0";
const NOTES: &str = "0-1";
const WEBSITE: &str = "1-1";

fn center_title(window: &mut Window) -> String {
    window
        .find("center-title")
        .label()
        .unwrap_or_default()
        .to_string()
}

fn last_space(settings_path: &Path) -> Space {
    Settings::load(settings_path, Path::new("/nonexistent")).last_space
}

#[gpui_kit::test]
fn switching_space_by_toggle_and_shortcut_remembers_last_space(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = f.settings_path.clone();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(is_selected(window, None, "Personal"));

        click_label(window, None, "Work", cx);
        assert!(is_selected(window, None, "Work"));
        assert!(!is_selected(window, None, "Personal"));
        assert_eq!(last_space(&path), Space::Work);

        window.press("cmd-1", cx);
        assert!(is_selected(window, None, "Personal"));
        assert_eq!(last_space(&path), Space::Personal);

        window.press("cmd-2", cx);
        assert!(is_selected(window, None, "Work"));
        assert_eq!(last_space(&path), Space::Work);
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = f.shell.read(cx).workspace().clone();
        assert_eq!(workspace.read(cx).space(), Space::Work);
    });
}

#[gpui_kit::test]
fn each_space_remembers_its_nav_selection(cx: &mut TestAppContext) {
    let f = open_shell(cx);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        // A fresh Space opens on its Inbox, showing the Project's Board.
        assert_eq!(center_title(window), "Inbox");
        assert!(window.try_find_by_label("Board").is_some());

        window.within("nav").click(NOTES, cx);
        assert_eq!(center_title(window), "Notes");
        assert!(window.try_find_by_label("Board").is_none());

        window.press("cmd-2", cx);
        assert_eq!(center_title(window), "Inbox");
        window.within("nav").click(JOURNAL, cx);
        assert_eq!(center_title(window), "Journal");

        window.press("cmd-1", cx);
        assert_eq!(center_title(window), "Notes");

        window.press("cmd-2", cx);
        assert_eq!(center_title(window), "Journal");
    })
    .unwrap();
}

#[gpui_kit::test]
fn nav_follows_projects_added_and_removed_on_disk(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let website = f.data_root.join("personal/projects/website");
    let change = Change::Project("website".into());

    fs::create_dir_all(&website).unwrap();
    fs::write(website.join("project.md"), "---\nname: Website\n---\n").unwrap();
    apply_change(cx, &f, change.clone());
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        // Projects sort Inbox first, then by slug: the new one is the second row.
        window.within("nav").click(WEBSITE, cx);
        assert_eq!(center_title(window), "Website");
    })
    .unwrap();

    // Deleting the open Project falls back to the Inbox.
    fs::remove_dir_all(&website).unwrap();
    apply_change(cx, &f, change);
    // No forced frame: the window only redraws if the change notified it.
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, _| {
        assert_eq!(center_title(window), "Inbox");
        assert!(window.within("nav").try_find(WEBSITE).is_none());
    })
    .unwrap();
}

/// Reports a change the way the watcher does.
fn apply_change(cx: &mut TestAppContext, f: &Fixture, change: Change) {
    cx.update(|cx| {
        let workspace = f.shell.read(cx).workspace().clone();
        workspace.update(cx, |workspace, cx| workspace.apply_change(change, cx));
    });
}

//! Shared setup for the app's UI integration tests.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use gpui_kit::test::{ElementSnapshot, TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, MouseButton, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};
use sett::{AppModel, Shell};
use sett_store::{Change, Settings};

pub struct Fixture {
    _dir: tempfile::TempDir,
    pub settings_path: PathBuf,
    pub data_root: PathBuf,
    pub window: AnyWindowHandle,
    pub shell: Entity<Shell>,
}

impl Fixture {
    pub fn model(&self, cx: &mut TestAppContext) -> Entity<AppModel> {
        cx.update(|cx| self.shell.read(cx).model().clone())
    }
}

pub fn open_shell(cx: &mut TestAppContext) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.toml");
    let settings = Settings::defaults(dir.path());
    let data_root = settings.data_root.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let (window, shell) = cx.update(|cx| {
        let model = AppModel::open_unwatched(settings, Some(settings_path.clone()), cx).unwrap();
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1024.), px(720.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(model, window, cx)),
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
pub fn click_label(
    window: &mut Window,
    scope: Option<&'static str>,
    label: &str,
    cx: &mut gpui_kit::App,
) {
    let target = find_label(window, scope, label);
    click_at(window, target, MouseButton::Left, cx);
}

/// Right-clicks the element with this accessible label.
pub fn right_click_label(
    window: &mut Window,
    scope: Option<&'static str>,
    label: &str,
    cx: &mut gpui_kit::App,
) {
    let target = find_label(window, scope, label);
    click_at(window, target, MouseButton::Right, cx);
}

fn click_at(
    window: &mut Window,
    target: ElementSnapshot,
    button: MouseButton,
    cx: &mut gpui_kit::App,
) {
    let center = target.bounds().center();
    window.pointer_move(center, None, cx);
    window.pointer_down(center, button, cx);
    window.pointer_up(center, button, cx);
}

pub fn find_label(
    window: &mut Window,
    scope: Option<&'static str>,
    label: &str,
) -> ElementSnapshot {
    match scope {
        Some(scope) => window.within(scope).find_by_label(label),
        None => window.find_by_label(label),
    }
}

pub fn center_title(window: &mut Window) -> String {
    window
        .find("center-title")
        .label()
        .unwrap_or_default()
        .to_string()
}

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

/// Writes a file in the personal Space and tells the app.
pub fn write(cx: &mut TestAppContext, f: &Fixture, rel: &str, text: &str, change: Change) {
    write_aged(cx, f, rel, text, 0, change);
}

/// Like [`write`], with the file `age` seconds old.
pub fn write_aged(
    cx: &mut TestAppContext,
    f: &Fixture,
    rel: &str,
    text: &str,
    age: u64,
    change: Change,
) {
    let path = f.data_root.join("personal").join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    if age > 0 {
        let when = SystemTime::now() - Duration::from_secs(age);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(change, cx)));
}

/// The change for a Note, in `project` or at the Space level.
pub fn note(project: Option<&str>, slug: &str) -> Change {
    Change::Note {
        project: project.map(str::to_string),
        slug: slug.to_string(),
    }
}

/// Lets the autosave write.
pub fn settle(cx: &mut TestAppContext) {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
}

/// Types `text` into whatever has focus.
pub fn input(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Presses each of the space-separated keystrokes in `keys`.
pub fn press(cx: &mut TestAppContext, f: &Fixture, keys: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        for key in keys.split_whitespace() {
            window.press(key, cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
}

/// Presses `key` `times` times.
pub fn press_n(cx: &mut TestAppContext, f: &Fixture, key: &str, times: usize) {
    press(cx, f, &vec![key; times].join(" "));
}

/// Whether anything in the window has this accessible label.
pub fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label(label).is_some()
    })
    .unwrap()
}

/// Whether the note body has something with this accessible label.
pub fn body_has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .within("note-body")
            .try_find_by_label(label)
            .is_some()
    })
    .unwrap()
}

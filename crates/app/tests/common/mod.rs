//! Shared setup for the app's UI integration tests.

#![allow(dead_code)]

use std::path::PathBuf;

use gpui_kit::test::{ElementSnapshot, TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, MouseButton, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};
use sett::{AppModel, Shell};
use sett_store::Settings;

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

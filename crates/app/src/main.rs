use std::path::Path;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use sett::{
    ArchiveProject, DeleteProject, EditProject, NewProject, Shell, ShowPersonal, ShowWork,
    UnarchiveProject, Workspace,
};
use sett_store::Settings;

actions!(sett, [Quit]);

fn main() {
    let settings_path = Settings::default_path();
    let home = std::env::var_os("HOME").unwrap_or_default();
    let settings = match &settings_path {
        Some(path) => Settings::load(path, Path::new(&home)),
        None => Settings::defaults(Path::new(&home)),
    };

    application().with_assets(sett::AppAssets).run(move |cx| {
        init(cx);
        sett::init(cx);

        // Bind before set_menus: menus copy shortcuts from the keymap at call time.
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus(vec![
            Menu::new("sett").items([MenuItem::action("Quit sett", Quit)]),
            Menu::new("Space").items([
                MenuItem::action("Personal", ShowPersonal),
                MenuItem::action("Work", ShowWork),
            ]),
            Menu::new("Project").items([
                MenuItem::action("New Project\u{2026}", NewProject),
                MenuItem::action("Edit Project\u{2026}", EditProject),
                MenuItem::separator(),
                MenuItem::action("Archive Project", ArchiveProject),
                MenuItem::action("Unarchive Project", UnarchiveProject),
                MenuItem::separator(),
                MenuItem::action("Delete Project\u{2026}", DeleteProject),
            ]),
        ]);

        let data_root = settings.data_root.clone();
        let workspace = match Workspace::open(settings.clone(), settings_path.clone(), cx) {
            Ok(workspace) => workspace,
            Err(err) => {
                eprintln!("sett: couldn't open {}: {err}", data_root.display());
                cx.quit();
                return;
            }
        };

        let bounds = Bounds::centered(None, size(px(1024.), px(720.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..TitleBar::window_options()
        };
        open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(workspace, window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}

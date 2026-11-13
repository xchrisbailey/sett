use std::path::Path;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use sett::{
    AppModel, ArchiveProject, DeleteProject, EditProject, GoBack, GoForward, NewProject, Selection,
    Shell, ShowPersonal, ShowWork, ToggleEditorMode, UnarchiveProject,
};
use sett_store::{LinkTarget, Settings};

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
            Menu::new("View").items([MenuItem::action("Toggle Raw Markdown", ToggleEditorMode)]),
            Menu::new("Go").items([
                MenuItem::action("Back", GoBack),
                MenuItem::action("Forward", GoForward),
            ]),
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
        let model = match AppModel::open(settings.clone(), settings_path.clone(), cx) {
            Ok(model) => model,
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
        let shell_model = model.clone();
        open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(shell_model, window, cx))
        })
        .expect("failed to open window");
        if std::env::args().any(|arg| arg == "--insert-menu") {
            sett::open_insert_menu_on_load(cx);
        }
        // After the Shell exists, so it sees the open request.
        let screen = arg_after("--screen");
        let open = arg_after("--open");
        model.update(cx, |m, cx| match (screen.as_deref(), open) {
            (Some("templates"), Some(slug)) => m.open_template(&slug, cx),
            (screen, Some(slug)) => {
                if let Some(screen) = screen {
                    eprintln!("sett: --screen {screen} is ignored with --open");
                }
                match m.index().resolve(&slug) {
                    LinkTarget::Found { item, .. } => m.open_item(item, cx),
                    LinkTarget::Broken => eprintln!("sett: --open {slug}: no such Item"),
                }
            }
            (Some("journal"), None) => m.select(Selection::Journal, cx),
            (Some("notes"), None) => m.select(Selection::Notes, cx),
            (Some("templates"), None) => m.select(Selection::Templates, cx),
            (Some(other), None) => {
                eprintln!("sett: --screen {other}: not journal, notes or templates")
            }
            (None, None) => {}
        });
        cx.activate(true);
    });
}

/// The value after `flag` on the command line. `scripts/screenshot` passes
/// `--open SLUG` (an Item, or a Template with `--screen templates`) and
/// `--screen journal|notes|templates` to choose what the window shows.
fn arg_after(flag: &str) -> Option<String> {
    let mut args = std::env::args().skip_while(|arg| arg != flag);
    args.next()?;
    args.next()
}

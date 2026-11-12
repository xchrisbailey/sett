//! Launching on an existing data root, the way `main` does (#13).

use std::fs;
use std::path::Path;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use sett::{AppModel, Shell};
use sett_store::{Settings, Space};

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

#[gpui_kit::test]
fn board_shows_its_cards_on_the_first_frame(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/sett");
    let mut settings = Settings::defaults(dir.path());
    copy_dir(&fixtures, &settings.data_root);
    settings.last_space = Space::Work;

    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let (window, _shell) = cx.update(|cx| {
        let model = AppModel::open_unwatched(settings, None, cx).unwrap();
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

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .within("col-Todo")
                .find("card-reply-to-recruiter")
                .label(),
            Some("Reply to recruiter")
        );
        assert_eq!(
            window
                .within("col-Doing")
                .find("card-expense-report")
                .label(),
            Some("Expense report")
        );
    })
    .unwrap();
}

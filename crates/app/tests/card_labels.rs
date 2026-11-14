//! UI integration tests for the Label chips on a Card.

use std::fs;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use sett::{AppModel, Shell};
use sett_store::{Change, Settings};

mod common;
use common::find_card;

/// Opens the Personal Inbox's Board with these Tasks on disk, each
/// `(slug, labels frontmatter line)`, all in Todo.
fn open_board(cx: &mut TestAppContext, tasks: &[(&str, &str)]) -> gpui_kit::AnyWindowHandle {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::defaults(dir.path());
    let tasks_dir = settings.data_root.join("personal/projects/inbox/tasks");
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let model = cx.update(|cx| AppModel::open_unwatched(settings, None, cx).unwrap());
    fs::create_dir_all(&tasks_dir).unwrap();
    for (slug, labels) in tasks {
        let text = format!("---\ntitle: {slug}\ncolumn: Todo\norder: 1\n{labels}\n---\n");
        fs::write(tasks_dir.join(format!("{slug}.md")), text).unwrap();
    }
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(Change::Project("inbox".into()), cx)
        })
    });
    let (window, _shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1280.), px(800.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(model, window, cx)),
        )
        .unwrap()
    });
    // Keep the temp dir alive for the test's length.
    std::mem::forget(dir);
    window
}

#[gpui_kit::test]
fn a_card_shows_three_chips_and_counts_the_rest(cx: &mut TestAppContext) {
    let window = open_board(
        cx,
        &[("many", "labels: [bug, sett, docs, ui, api]"), ("none", "")],
    );
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let row = window.within("card-labels-many");
        for shown in ["bug", "sett", "docs"] {
            assert!(row.try_find_by_label(shown).is_some(), "{shown} missing");
        }
        for hidden in ["ui", "api"] {
            assert!(row.try_find_by_label(hidden).is_none(), "{hidden} shown");
        }
        assert!(row.try_find_by_label("+2").is_some());

        assert!(window.try_find("card-labels-none").is_none());
        assert_eq!(
            find_card(window, "many").description(),
            Some("Labels: bug, sett, docs, ui, api")
        );
        assert_eq!(find_card(window, "none").description(), None);
    })
    .unwrap();
}

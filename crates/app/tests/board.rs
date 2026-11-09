//! UI integration tests for a Project's Board: quick-add, moving Cards and
//! the last Column's 14-day window.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Days, Local};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions, px,
    size,
};
use sett::{AppModel, Shell};
use sett_store::{Change, Settings};

struct Fixture {
    _dir: tempfile::TempDir,
    tasks_dir: PathBuf,
    window: AnyWindowHandle,
    shell: Entity<Shell>,
}

/// Opens the shell on the Personal Inbox's Board with `tasks` already on disk,
/// each `(slug, column, order, done)`.
fn open_board(cx: &mut TestAppContext, tasks: &[(&str, &str, f64, Option<String>)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::defaults(dir.path());
    let tasks_dir = settings.data_root.join("personal/projects/inbox/tasks");
    // Store::open runs first-run setup only for a missing root, so create it
    // through the AppModel first and add the Tasks afterwards.
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let model = cx.update(|cx| AppModel::open_unwatched(settings, None, cx).unwrap());
    fs::create_dir_all(&tasks_dir).unwrap();
    for (slug, column, order, done) in tasks {
        write_task(&tasks_dir, slug, column, *order, done.as_deref());
    }
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(Change::Project("inbox".into()), cx)
        })
    });
    let (window, shell) = cx.update(|cx| {
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
    Fixture {
        _dir: dir,
        tasks_dir,
        window,
        shell,
    }
}

/// A YAML comment never survives a rewrite by sett, so it marks a file as
/// untouched.
fn write_task(dir: &Path, slug: &str, column: &str, order: f64, done: Option<&str>) {
    let done = done.map(|d| format!("done: {d}\n")).unwrap_or_default();
    let text =
        format!("---\n# by hand\ntitle: {slug}\ncolumn: {column}\norder: {order}\n{done}---\n");
    fs::write(dir.join(format!("{slug}.md")), text).unwrap();
}

fn days_ago(days: u64) -> String {
    let date = Local::now().date_naive() - Days::new(days);
    date.format("%Y-%m-%d").to_string()
}

fn model(cx: &mut TestAppContext, f: &Fixture) -> Entity<AppModel> {
    cx.update(|cx| f.shell.read(cx).model().clone())
}

#[gpui_kit::test]
fn quick_add_creates_a_card_in_that_column(cx: &mut TestAppContext) {
    let f = open_board(cx, &[]);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("col-Doing").click("quick-add", cx);
        window.input("Write docs", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let card = window.within("col-Doing").find("card-write-docs");
        assert_eq!(card.label(), Some("Write docs"));
        assert!(
            window
                .within("col-Todo")
                .try_find("card-write-docs")
                .is_none()
        );
        assert_eq!(
            window.within("col-Doing").find("quick-add").value(),
            Some("")
        );
    })
    .unwrap();
    let text = fs::read_to_string(f.tasks_dir.join("write-docs.md")).unwrap();
    assert!(text.contains("column: Doing"), "{text}");
}

#[gpui_kit::test]
fn moving_a_card_rewrites_only_that_tasks_file(cx: &mut TestAppContext) {
    let f = open_board(
        cx,
        &[
            ("a", "Todo", 1.0, None),
            ("b", "Todo", 2.0, None),
            ("c", "Doing", 1.0, None),
        ],
    );
    let untouched: Vec<String> = ["a", "b"]
        .iter()
        .map(|s| fs::read_to_string(f.tasks_dir.join(format!("{s}.md"))).unwrap())
        .collect();

    let model = model(cx, &f);
    cx.update(|cx| model.update(cx, |w, cx| w.move_task("inbox", "c", "Todo", Some("b"), cx)))
        .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let cards: Vec<String> = window
            .within("col-Todo")
            .find_all_by_role(gpui_kit::Role::ListItem)
            .iter()
            .filter_map(|c| c.label().map(str::to_string))
            .collect();
        assert_eq!(cards, ["a", "c", "b"]);
        assert!(window.within("col-Doing").try_find("card-c").is_none());
    })
    .unwrap();
    for (slug, before) in ["a", "b"].iter().zip(&untouched) {
        let now = fs::read_to_string(f.tasks_dir.join(format!("{slug}.md"))).unwrap();
        assert_eq!(&now, before, "{slug} was rewritten");
    }
    let moved = fs::read_to_string(f.tasks_dir.join("c.md")).unwrap();
    assert!(
        moved.contains("column: Todo") && moved.contains("order: 1.5"),
        "{moved}"
    );
}

#[gpui_kit::test]
fn old_done_tasks_are_hidden_until_show_all(cx: &mut TestAppContext) {
    let f = open_board(
        cx,
        &[
            ("recent", "Done", 1.0, Some(days_ago(14))),
            ("old", "Done", 2.0, Some(days_ago(15))),
        ],
    );

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let done = |window: &mut gpui_kit::Window, slug: &str| {
            window
                .within("col-Done")
                .try_find(format!("card-{slug}"))
                .is_some()
        };
        assert!(done(window, "recent"));
        assert!(!done(window, "old"));

        window.within("col-Done").click("show-all", cx);
        assert!(done(window, "old"));

        window.within("col-Done").click("show-all", cx);
        assert!(!done(window, "old"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn dragging_a_card_onto_a_column_moves_it_there(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, None), ("b", "Doing", 1.0, None)]);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.drag_to("card-a", "card-b", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.within("col-Doing").try_find("card-a").is_some());
        assert!(window.within("col-Todo").try_find("card-a").is_none());
    })
    .unwrap();
    let moved = fs::read_to_string(f.tasks_dir.join("a.md")).unwrap();
    assert!(
        moved.contains("column: Doing") && moved.contains("order: 0"),
        "{moved}"
    );
}

//! UI integration tests for the Task view: opening it from a Card, going back
//! to the Board, and editing the Task's Column and due date.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui_kit::test::{
    TestAppContextExt as _, TestEventExt as _, TestQueryExt as _, TestWindowExt as _,
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, MouseButton, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use sett::{AppModel, Shell};
use sett_store::{Change, Settings};

struct Fixture {
    _dir: tempfile::TempDir,
    tasks_dir: PathBuf,
    window: AnyWindowHandle,
}

/// Opens the shell on the Personal Inbox's Board with `tasks` on disk, each
/// `(slug, column, order, extra frontmatter)`.
fn open_board(cx: &mut TestAppContext, tasks: &[(&str, &str, f64, &str)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::defaults(dir.path());
    let tasks_dir = settings.data_root.join("personal/projects/inbox/tasks");
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let model = cx.update(|cx| AppModel::open_unwatched(settings, None, cx).unwrap());
    fs::create_dir_all(&tasks_dir).unwrap();
    for (slug, column, order, extra) in tasks {
        // A YAML comment never survives a rewrite by sett, so it marks a file
        // as untouched.
        let text = format!(
            "---\n# by hand\ntitle: {slug}\ncolumn: {column}\norder: {order}\n{extra}---\nSome __bold__ body\n"
        );
        fs::write(tasks_dir.join(format!("{slug}.md")), text).unwrap();
    }
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(Change::Project("inbox".into()), cx)
        })
    });
    let (window, _) = cx.update(|cx| {
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
    }
}

fn snapshot(path: &Path) -> (Vec<u8>, SystemTime) {
    (
        fs::read(path).unwrap(),
        fs::metadata(path).unwrap().modified().unwrap(),
    )
}

fn task_path(f: &Fixture, slug: &str) -> PathBuf {
    f.tasks_dir.join(format!("{slug}.md"))
}

fn open_task(cx: &mut TestAppContext, f: &Fixture, slug: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("card-{slug}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn showing_board(cx: &mut TestAppContext, f: &Fixture) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let board = window.try_find("add-column").is_some();
        let task = window.try_find("task-view").is_some();
        assert_ne!(board, task, "exactly one of Board and Task view shows");
        board
    })
    .unwrap()
}

#[gpui_kit::test]
fn clicking_a_card_opens_its_task(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);

    open_task(cx, &f, "a");

    assert!(!showing_board(cx, &f));
    cx.update_window(f.window, |_, window, _| {
        assert_eq!(window.find("task-title").value(), Some("a"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn escape_returns_to_the_board(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);
    open_task(cx, &f, "a");

    cx.update_window(f.window, |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(showing_board(cx, &f));

    // Also from inside the body, once the editor has focus.
    open_task(cx, &f, "a");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(showing_board(cx, &f));
}

#[gpui_kit::test]
fn the_breadcrumb_returns_to_the_board(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);
    open_task(cx, &f, "a");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        // The crumbs carry no ids of their own; the Project's is first.
        let bounds = window.find("task-breadcrumb").bounds();
        let at = point(bounds.left() + px(4.), bounds.center().y);
        window.pointer_move(at, None, cx);
        window.pointer_down(at, MouseButton::Left, cx);
        window.pointer_up(at, MouseButton::Left, cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(showing_board(cx, &f));
}

#[gpui_kit::test]
fn opening_a_task_without_editing_writes_nothing(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "due: 2026-03-04\n")]);
    let before = snapshot(&task_path(&f, "a"));

    open_task(cx, &f, "a");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.click("task-title", cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();

    assert!(showing_board(cx, &f));
    assert_eq!(snapshot(&task_path(&f, "a")), before);
}

fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[gpui_kit::test]
async fn choosing_a_column_moves_the_task_like_a_drag(cx: &mut TestAppContext) {
    let f = open_board(
        cx,
        &[
            ("a", "Todo", 1.0, ""),
            ("b", "Todo", 2.0, ""),
            ("c", "Done", 1.0, "done: 2026-01-02\n"),
        ],
    );
    let untouched = [snapshot(&task_path(&f, "b")), snapshot(&task_path(&f, "c"))];
    open_task(cx, &f, "a");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("task-column").value(), Some("Todo"));
        window.within("task-column").click("input", cx);
        // Todo, Doing, Done.
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    // The Select commits after the key dispatch returns.
    cx.wait_for(f.window, Duration::from_millis(500), |window, _| {
        window.find("task-column").value() == Some("Done")
    })
    .await;

    let moved = fs::read_to_string(task_path(&f, "a")).unwrap();
    assert!(
        moved.contains("column: Done")
            && moved.contains("order: 2")
            && moved.contains(&format!("done: {}", today())),
        "{moved}"
    );
    assert_eq!(
        [snapshot(&task_path(&f, "b")), snapshot(&task_path(&f, "c"))],
        untouched
    );
    assert!(!showing_board(cx, &f), "the Task stays open");
}

#[gpui_kit::test]
fn setting_and_clearing_the_due_date_writes_due(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);
    open_task(cx, &f, "a");
    let today = today();

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("task-due", cx);
        window.render_frame(cx);
        let day = window.find_by_label(&today).bounds().center();
        window.pointer_move(day, None, cx);
        window.pointer_down(day, MouseButton::Left, cx);
        window.pointer_up(day, MouseButton::Left, cx);
    })
    .unwrap();
    cx.run_until_parked();
    let text = fs::read_to_string(task_path(&f, "a")).unwrap();
    assert!(text.contains(&format!("due: {today}")), "{text}");
    assert!(text.contains("Some __bold__ body"), "{text}");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("task-due", cx);
        window.press("backspace", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let text = fs::read_to_string(task_path(&f, "a")).unwrap();
    assert!(!text.contains("due:"), "{text}");
}

#[gpui_kit::test]
fn escape_puts_focus_back_on_the_card_and_enter_reopens_it(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);
    open_task(cx, &f, "a");
    cx.update_window(f.window, |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(showing_board(cx, &f));

    cx.update_window(f.window, |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!showing_board(cx, &f));
}

#[gpui_kit::test]
fn switching_space_closes_the_task(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("a", "Todo", 1.0, "")]);
    // The Work Inbox has a Task with the same slug, which mustn't show.
    let root = f.tasks_dir.ancestors().nth(4).unwrap();
    let work_tasks = root.join("work/projects/inbox/tasks");
    fs::create_dir_all(&work_tasks).unwrap();
    fs::write(
        work_tasks.join("a.md"),
        "---\ntitle: a\ncolumn: Todo\n---\n",
    )
    .unwrap();
    open_task(cx, &f, "a");

    cx.update_window(f.window, |_, window, cx| window.press("cmd-2", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(showing_board(cx, &f));
}

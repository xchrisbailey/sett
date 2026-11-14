//! UI integration tests for the Board's Label filter.

use std::fs;
use std::path::PathBuf;

use chrono::{Days, Local};
use gpui_kit::test::{TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Role, TestAppContext, WindowBounds, WindowOptions,
    px, size,
};
use sett::{AppModel, Selection, Shell};
use sett_store::{Change, Settings};

mod common;
use common::click_label;

struct Fixture {
    _dir: tempfile::TempDir,
    tasks_dir: PathBuf,
    window: AnyWindowHandle,
    shell: Entity<Shell>,
}

/// A Task on disk: `(slug, column, labels frontmatter line, done date)`.
type Seed<'a> = (&'a str, &'a str, &'a str, Option<String>);

/// Opens the Personal Inbox's Board with these Tasks on disk.
fn open_board(cx: &mut TestAppContext, tasks: &[Seed]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::defaults(dir.path());
    let tasks_dir = settings.data_root.join("personal/projects/inbox/tasks");
    cx.update(|cx| {
        gpui_kit::init(cx);
        sett::init(cx);
    });
    let model = cx.update(|cx| AppModel::open_unwatched(settings, None, cx).unwrap());
    fs::create_dir_all(&tasks_dir).unwrap();
    for (ix, (slug, column, labels, done)) in tasks.iter().enumerate() {
        write_task(&tasks_dir, slug, column, ix, labels, done.as_deref());
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

fn write_task(
    dir: &std::path::Path,
    slug: &str,
    column: &str,
    order: usize,
    labels: &str,
    done: Option<&str>,
) {
    let done = done.map(|d| format!("done: {d}\n")).unwrap_or_default();
    let text =
        format!("---\ntitle: {slug}\ncolumn: {column}\norder: {order}\n{labels}\n{done}---\n");
    fs::write(dir.join(format!("{slug}.md")), text).unwrap();
}

fn days_ago(days: u64) -> String {
    let date = Local::now().date_naive() - Days::new(days);
    date.format("%Y-%m-%d").to_string()
}

fn model(cx: &mut TestAppContext, f: &Fixture) -> Entity<AppModel> {
    cx.update(|cx| f.shell.read(cx).model().clone())
}

/// Runs `body` on a freshly rendered window.
fn with_window<R>(
    cx: &mut TestAppContext,
    f: &Fixture,
    body: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App) -> R,
) -> R {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        body(window, cx)
    })
    .unwrap()
}

fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    with_window(cx, f, |window, _| window.try_find_by_label(label).is_some())
}

/// Opens the picker from whichever button is showing and toggles its
/// `row`th option with the keyboard, since the popup's rows aren't in the
/// test query tree. Escape then closes it, as a user would.
fn toggle(cx: &mut TestAppContext, f: &Fixture, trigger: &str, row: usize) {
    with_window(cx, f, |window, cx| click_label(window, None, trigger, cx));
    cx.run_until_parked();
    with_window(cx, f, |window, cx| {
        for _ in 0..=row {
            window.press("down", cx);
        }
        window.press("enter", cx);
    });
    cx.run_until_parked();
    with_window(cx, f, |window, cx| window.press("escape", cx));
    cx.run_until_parked();
}

/// Slugs of the Cards in a Column, top to bottom.
fn cards(cx: &mut TestAppContext, f: &Fixture, column: &'static str) -> Vec<String> {
    with_window(cx, f, |window, _| {
        window
            .within(column)
            .find_all_by_role(Role::ListItem)
            .iter()
            .filter_map(|c| c.label().map(str::to_string))
            .collect()
    })
}

fn header_count(cx: &mut TestAppContext, f: &Fixture, column: &'static str) -> String {
    with_window(cx, f, |window, _| {
        window
            .within(column)
            .find("column-count")
            .label()
            .unwrap_or_default()
            .to_string()
    })
}

const SEEDS: [Seed; 4] = [
    ("fix-crash", "Todo", "labels: [Bug]", None),
    ("write-docs", "Todo", "labels: [docs, sett]", None),
    ("plain", "Todo", "", None),
    ("ship", "Doing", "labels: [bug, ui]", None),
];

#[gpui_kit::test]
fn picking_labels_shows_tasks_with_any_of_them_ignoring_case(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    assert_eq!(cards(cx, &f, "col-Todo").len(), 3);

    toggle(cx, &f, "Filter by Label", 0);
    assert_eq!(cards(cx, &f, "col-Todo"), ["fix-crash"]);
    assert_eq!(cards(cx, &f, "col-Doing"), ["ship"]);
    assert_eq!(header_count(cx, &f, "col-Todo"), "1 of 3");

    toggle(cx, &f, "Labels, 1 picked", 1);
    assert_eq!(cards(cx, &f, "col-Todo"), ["fix-crash", "write-docs"]);
    assert_eq!(header_count(cx, &f, "col-Todo"), "2 of 3");
}

#[gpui_kit::test]
fn a_column_without_matches_stays_and_says_so(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    toggle(cx, &f, "Filter by Label", 2);

    assert!(cards(cx, &f, "col-Doing").is_empty());
    assert_eq!(header_count(cx, &f, "col-Doing"), "0 of 1");
    with_window(cx, &f, |window, _| {
        assert!(
            window
                .within("col-Doing")
                .try_find_by_label("No matching Tasks")
                .is_some()
        );
        assert!(window.within("col-Doing").try_find("quick-add").is_some());
    });
}

#[gpui_kit::test]
fn x_and_clear_unpick(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    toggle(cx, &f, "Filter by Label", 0);
    toggle(cx, &f, "Labels, 1 picked", 1);
    assert_eq!(cards(cx, &f, "col-Todo"), ["fix-crash", "write-docs"]);

    with_window(&mut *cx, &f, |window, cx| {
        click_label(window, None, "Remove Bug from the filter", cx)
    });
    assert_eq!(cards(cx, &f, "col-Todo"), ["write-docs"]);
    assert!(has_label(cx, &f, "Labels, 1 picked"));

    with_window(&mut *cx, &f, |window, cx| {
        click_label(window, None, "Clear", cx)
    });
    assert_eq!(cards(cx, &f, "col-Todo").len(), 3);
    assert!(has_label(cx, &f, "Filter by Label"));
    assert_eq!(header_count(cx, &f, "col-Todo"), "3");
}

#[gpui_kit::test]
fn a_project_without_labels_has_a_disabled_filter(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("plain", "Todo", "", None)]);
    with_window(cx, &f, |window, _| {
        assert_eq!(
            window.find_by_label("Filter by Label").disabled(),
            Some(true)
        );
    });
}

/// Types a title into a Column's quick add and presses Enter.
fn quick_add(cx: &mut TestAppContext, f: &Fixture, column: &'static str, title: &str) {
    with_window(cx, f, |window, cx| {
        window.within(column).click("quick-add", cx);
        window.input(title, cx);
        window.press("enter", cx);
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn quick_add_takes_a_single_picked_label_and_none_of_several(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    toggle(cx, &f, "Filter by Label", 1);

    quick_add(cx, &f, "col-Todo", "One label");
    assert!(cards(cx, &f, "col-Todo").contains(&"One label".to_string()));
    let text = fs::read_to_string(f.tasks_dir.join("one-label.md")).unwrap();
    assert!(text.contains("docs"), "{text}");

    toggle(cx, &f, "Labels, 1 picked", 0);

    quick_add(cx, &f, "col-Todo", "Two labels");
    assert!(cards(cx, &f, "col-Todo").contains(&"Two labels".to_string()));
    let text = fs::read_to_string(f.tasks_dir.join("two-labels.md")).unwrap();
    assert!(!text.contains("labels:"), "{text}");

    // It only stays until the picked set changes again.
    toggle(cx, &f, "Labels, 2 picked", 2);
    assert!(!cards(cx, &f, "col-Todo").contains(&"Two labels".to_string()));
}

#[gpui_kit::test]
fn the_filter_survives_opening_a_task_and_resets_on_a_project_switch(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    toggle(cx, &f, "Filter by Label", 0);

    with_window(cx, &f, |window, cx| {
        window.click("card-fix-crash", cx);
    });
    cx.run_until_parked();
    with_window(cx, &f, |window, cx| window.press("escape", cx));
    cx.run_until_parked();
    assert_eq!(cards(cx, &f, "col-Todo"), ["fix-crash"]);

    // Leaving the Board and coming back is a Project switch.
    let model = model(cx, &f);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Journal, cx)));
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Project("inbox".into()), cx)));
    cx.run_until_parked();
    assert!(has_label(cx, &f, "Filter by Label"));
    assert_eq!(cards(cx, &f, "col-Todo").len(), 3);
}

#[gpui_kit::test]
fn a_label_that_vanishes_stays_picked(cx: &mut TestAppContext) {
    let f = open_board(cx, &SEEDS);
    toggle(cx, &f, "Filter by Label", 1);

    fs::remove_file(f.tasks_dir.join("write-docs.md")).unwrap();
    let model = model(cx, &f);
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(
                Change::Task {
                    project: "inbox".into(),
                    slug: "write-docs".into(),
                },
                cx,
            )
        })
    });
    cx.run_until_parked();

    assert!(has_label(cx, &f, "Labels, 1 picked"));
    assert!(has_label(cx, &f, "Remove docs from the filter"));
    assert!(cards(cx, &f, "col-Todo").is_empty());
}

#[gpui_kit::test]
fn show_all_counts_only_matching_older_tasks(cx: &mut TestAppContext) {
    let f = open_board(
        cx,
        &[
            ("recent", "Done", "labels: [bug]", Some(days_ago(1))),
            ("old-bug", "Done", "labels: [bug]", Some(days_ago(30))),
            ("old-doc", "Done", "labels: [docs]", Some(days_ago(30))),
            ("old-doc-2", "Done", "labels: [docs]", Some(days_ago(31))),
        ],
    );
    toggle(cx, &f, "Filter by Label", 0);
    assert!(has_label(cx, &f, "Show all (1 older)"));
    assert_eq!(cards(cx, &f, "col-Done"), ["recent"]);
}

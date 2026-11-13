//! UI integration tests for the Callout line on a Card.

use std::fs;
use std::path::{Path, PathBuf};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ElementId, Entity, TestAppContext, WindowBounds,
    WindowOptions, px, size,
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
/// each `(slug, body)`, all in Todo.
fn open_board(cx: &mut TestAppContext, tasks: &[(&str, &str)]) -> Fixture {
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
    for (slug, body) in tasks {
        write_task(&tasks_dir, slug, body);
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

fn write_task(dir: &Path, slug: &str, body: &str) {
    let text = format!("---\ntitle: {slug}\ncolumn: Todo\norder: 1\n---\n{body}");
    fs::write(dir.join(format!("{slug}.md")), text).unwrap();
}

fn description(
    cx: &mut TestAppContext,
    f: &Fixture,
    slug: &str,
) -> (Option<String>, Option<String>) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let card = window.find(ElementId::Name(format!("card-{slug}").into()));
        let out = (
            card.label().map(str::to_string),
            card.description().map(str::to_string),
        );
        out
    })
    .unwrap()
}

#[gpui_kit::test]
fn a_card_with_a_callout_keeps_its_title_and_describes_the_callout(cx: &mut TestAppContext) {
    let body = "> [!WARNING]\n> Breaks on UNC paths like \\\\server\\share.\n> Needs a Windows VM to reproduce.\n> Third line.\n";
    let f = open_board(cx, &[("paths", body)]);
    let (label, desc) = description(cx, &f, "paths");
    assert_eq!(label.as_deref(), Some("paths"));
    assert_eq!(
        desc.as_deref(),
        Some(
            "Warning: Breaks on UNC paths like \\\\server\\share. Needs a Windows VM to reproduce.\u{2026}"
        )
    );
}

#[gpui_kit::test]
fn a_callout_of_at_most_two_lines_has_no_ellipsis(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("t", "> [!TIP]\n> One.\n> Two.\n")]);
    let (_, desc) = description(cx, &f, "t");
    assert_eq!(desc.as_deref(), Some("Tip: One. Two."));
}

#[gpui_kit::test]
fn a_card_without_a_callout_has_no_description(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("plain", "Just text.\n\n> a quote\n")]);
    let (label, desc) = description(cx, &f, "plain");
    assert_eq!(label.as_deref(), Some("plain"));
    assert_eq!(desc, None);
}

#[gpui_kit::test]
fn an_empty_first_callout_is_skipped_for_the_next_one(cx: &mut TestAppContext) {
    let body = "> [!NOTE]\n\n> [!CAUTION]\n> Real one.\n";
    let f = open_board(cx, &[("t", body)]);
    let (_, desc) = description(cx, &f, "t");
    assert_eq!(desc.as_deref(), Some("Caution: Real one."));
}

#[gpui_kit::test]
fn a_callout_changed_on_disk_updates_the_card(cx: &mut TestAppContext) {
    let f = open_board(cx, &[("t", "> [!NOTE]\n> Before.\n")]);
    assert_eq!(description(cx, &f, "t").1.as_deref(), Some("Note: Before."));

    write_task(&f.tasks_dir, "t", "> [!TIP]\n> After.\n");
    let model = model(cx, &f);
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(
                Change::Task {
                    project: "inbox".into(),
                    slug: "t".into(),
                },
                cx,
            )
        })
    });
    cx.run_until_parked();
    assert_eq!(description(cx, &f, "t").1.as_deref(), Some("Tip: After."));

    write_task(&f.tasks_dir, "t", "No callout now.\n");
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(
                Change::Task {
                    project: "inbox".into(),
                    slug: "t".into(),
                },
                cx,
            )
        })
    });
    cx.run_until_parked();
    assert_eq!(description(cx, &f, "t").1, None);
}

fn model(cx: &mut TestAppContext, f: &Fixture) -> Entity<AppModel> {
    cx.update(|cx| f.shell.read(cx).model().clone())
}

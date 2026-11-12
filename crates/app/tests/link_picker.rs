//! UI integration tests for the Link Picker: typing `[[` in an Item's body,
//! filtering and ordering the Items, picking with keys or the mouse, closing
//! without losing text, and undo. They assert on the saved file.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use common::{Fixture, click_label, open_shell};
use gpui_kit::test::{TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{AppContext as _, Role, TestAppContext};
use sett::Selection;
use sett_store::{Change, ItemId};

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

/// Writes a file in the personal Space, `age` seconds old, and tells the app.
fn write(cx: &mut TestAppContext, f: &Fixture, rel: &str, text: &str, age: u64, change: Change) {
    let path = f.data_root.join("personal").join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    let when = SystemTime::now() - Duration::from_secs(age);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(when)
        .unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(change, cx)));
}

fn note(project: Option<&str>, slug: &str) -> Change {
    Change::Note {
        project: project.map(str::to_string),
        slug: slug.to_string(),
    }
}

/// A Space with a few Items, then the Note "Plan" open with the caret on a
/// new empty line under its first line.
fn on_a_new_line(cx: &mut TestAppContext) -> (Fixture, PathBuf) {
    let f = open_shell(cx);
    let plan = "---\ntitle: Plan\n---\nFirst line\n";
    write(cx, &f, "notes/plan.md", plan, 900, note(None, "plan"));
    let notes = [
        ("quotes", "Quotes", 500),
        ("quick-start", "Quick start", 100),
        ("ideas", "Ideas on quotes", 10),
        ("garden", "Garden", 50),
    ];
    for (slug, title, age) in notes {
        let text = format!("---\ntitle: {title}\n---\nBody\n");
        write(
            cx,
            &f,
            &format!("notes/{slug}.md"),
            &text,
            age,
            note(None, slug),
        );
    }
    // A Project with a Task, and an archived Project whose Items are left out.
    write(
        cx,
        &f,
        "projects/kitchen-reno/project.md",
        "---\nname: Kitchen reno\n---\n",
        10,
        Change::Project("kitchen-reno".into()),
    );
    write(
        cx,
        &f,
        "projects/kitchen-reno/tasks/measure.md",
        "---\ntitle: Measure\n---\n",
        20,
        Change::Task {
            project: "kitchen-reno".into(),
            slug: "measure".into(),
        },
    );
    write(
        cx,
        &f,
        "projects/old/project.md",
        "---\nname: Old\narchived: true\n---\n",
        10,
        Change::Project("old".into()),
    );
    write(
        cx,
        &f,
        "projects/old/tasks/ancient.md",
        "---\ntitle: Ancient\n---\n",
        20,
        Change::Task {
            project: "old".into(),
            slug: "ancient".into(),
        },
    );
    // Two Items share the slug "shared"; the Space-level Note wins.
    write(
        cx,
        &f,
        "notes/shared.md",
        "---\ntitle: Shared top\n---\n",
        30,
        note(None, "shared"),
    );
    write(
        cx,
        &f,
        "projects/kitchen-reno/notes/shared.md",
        "---\ntitle: Shared nested\n---\n",
        40,
        note(Some("kitchen-reno"), "shared"),
    );

    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Notes, cx)));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let path = f.data_root.join("personal/notes/plan.md");
    (f, path)
}

fn input(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn press(cx: &mut TestAppContext, f: &Fixture, keys: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        for key in keys.split_whitespace() {
            window.press(key, cx);
        }
    })
    .unwrap();
    cx.run_until_parked();
}

fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label(label).is_some()
    })
    .unwrap()
}

fn picker_open(cx: &mut TestAppContext, f: &Fixture) -> bool {
    has_label(cx, f, "Link Picker")
}

/// The titles of the picker's rows, top to bottom.
fn rows(cx: &mut TestAppContext, f: &Fixture) -> Vec<String> {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .within("block-editor-link-picker")
            .find_all_by_role(Role::ListBoxOption)
            .iter()
            .filter_map(|n| n.label().map(str::to_string))
            .collect()
    })
    .unwrap()
}

fn saved_body(cx: &mut TestAppContext, path: &PathBuf) -> String {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    let text = fs::read_to_string(path).unwrap();
    text.split_once("---\n")
        .and_then(|(_, rest)| rest.split_once("---\n"))
        .map(|(_, body)| body.to_string())
        .unwrap_or(text)
}

#[gpui_kit::test]
fn typing_two_brackets_opens_it_mid_line(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    assert!(!picker_open(cx, &f));
    input(cx, &f, "see [[");
    assert!(picker_open(cx, &f));
}

#[gpui_kit::test]
fn typing_the_brackets_one_at_a_time_opens_it(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[");
    assert!(!picker_open(cx, &f));
    input(cx, &f, "[");
    assert!(picker_open(cx, &f));
}

#[gpui_kit::test]
fn it_does_not_open_inside_inline_code(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "`ab`");
    press(cx, &f, "left");
    input(cx, &f, "[[");
    assert!(!picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n`a[[b`\n");
}

#[gpui_kit::test]
fn it_does_not_open_inside_a_code_block(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "```");
    press(cx, &f, "enter");
    input(cx, &f, "[[");
    assert!(!picker_open(cx, &f));
}

#[gpui_kit::test]
fn the_current_item_and_archived_items_are_not_listed(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[");
    let titles = rows(cx, &f);
    assert!(titles.contains(&"Garden".to_string()), "{titles:?}");
    assert!(titles.contains(&"Measure".to_string()), "{titles:?}");
    assert!(!titles.contains(&"Plan".to_string()), "{titles:?}");
    assert!(!titles.contains(&"Ancient".to_string()), "{titles:?}");
}

#[gpui_kit::test]
fn an_empty_query_lists_the_most_recently_changed_first(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[");
    assert_eq!(
        rows(cx, &f),
        [
            "Ideas on quotes",
            "Measure",
            "Shared top",
            "Shared nested",
            "Garden",
            "Quick start",
            "Quotes"
        ]
    );
}

#[gpui_kit::test]
fn typing_filters_by_title_and_by_slug(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[GARD");
    assert_eq!(rows(cx, &f), ["Garden", &create_row("GARD")]);
    press(cx, &f, "backspace backspace backspace backspace");
    // The slug `quick-start` matches though the title has no hyphen.
    input(cx, &f, "k-st");
    assert_eq!(rows(cx, &f), ["Quick start", &create_row("k-st")]);
}

#[gpui_kit::test]
fn title_prefix_matches_come_first_then_the_most_recent(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[qu");
    // Prefix matches (Quick start is newer than Quotes), then "Ideas on
    // quotes", which merely contains the query.
    assert_eq!(
        rows(cx, &f),
        [
            "Quick start",
            "Quotes",
            "Ideas on quotes",
            &create_row("qu")
        ]
    );
}

#[gpui_kit::test]
fn enter_picks_the_highlighted_item(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "see [[qu");
    press(cx, &f, "down enter");
    assert!(!picker_open(cx, &f));
    input(cx, &f, " now");
    assert_eq!(saved_body(cx, &path), "First line\n\nsee [[quotes]] now\n");
}

#[gpui_kit::test]
fn tab_picks_the_highlighted_item(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[gard");
    press(cx, &f, "tab");
    assert_eq!(saved_body(cx, &path), "First line\n\n[[garden]]\n");
}

#[gpui_kit::test]
fn arrows_wrap_around(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[qu");
    // Up from the first row wraps to the last, the create row, then up once
    // more is "Ideas on quotes".
    press(cx, &f, "up up enter");
    assert_eq!(saved_body(cx, &path), "First line\n\n[[ideas]]\n");
}

#[gpui_kit::test]
fn clicking_a_row_picks_it(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, Some("block-editor-link-picker"), "Garden", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n[[garden]]\n");
}

#[gpui_kit::test]
fn one_undo_returns_to_the_typed_query(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[gard");
    press(cx, &f, "enter");
    press(cx, &f, "secondary-z");
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[gard\n");
}

#[gpui_kit::test]
fn escape_closes_it_and_keeps_the_typed_text(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[gard");
    press(cx, &f, "escape");
    assert!(!picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[gard\n");
}

#[gpui_kit::test]
fn typing_closing_brackets_closes_it(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[gard]");
    assert!(picker_open(cx, &f));
    input(cx, &f, "]");
    assert!(!picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n[[gard]]\n");
}

#[gpui_kit::test]
fn moving_the_caret_out_closes_it(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[gard");
    press(cx, &f, "home");
    assert!(!picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[gard\n");
}

#[gpui_kit::test]
fn an_unreachable_row_is_dimmed_with_a_hint_and_cannot_be_picked(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[shared");
    assert_eq!(
        rows(cx, &f),
        ["Shared top", "Shared nested", &create_row("shared")]
    );
    assert!(has_label(cx, &f, "Another Item has the slug `shared`"));
    // Down skips the unreachable row to the create row, and again wraps back
    // to the first.
    press(cx, &f, "down down");
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(
            window,
            Some("block-editor-link-picker"),
            "Shared nested",
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert!(picker_open(cx, &f));
    press(cx, &f, "enter");
    assert_eq!(saved_body(cx, &path), "First line\n\n[[shared]]\n");
}

#[gpui_kit::test]
fn a_blank_query_with_no_match_shows_a_message_and_stays_open(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[   ");
    assert!(picker_open(cx, &f));
    assert!(has_label(cx, &f, "No matching Items"));
    // Backspacing brings the rows back.
    press(cx, &f, "backspace backspace backspace");
    assert!(!has_label(cx, &f, "No matching Items"));
    assert!(picker_open(cx, &f));
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[\n");
}

#[gpui_kit::test]
fn enter_with_nothing_to_pick_closes_it_and_starts_a_new_line(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[   ");
    assert!(has_label(cx, &f, "No matching Items"));
    press(cx, &f, "enter");
    assert!(!picker_open(cx, &f));
    input(cx, &f, "x");
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[   \n\nx\n");
}

#[gpui_kit::test]
fn an_open_picker_lists_an_item_created_meanwhile(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[gizm");
    assert!(!rows(cx, &f).contains(&"Gizmo".to_string()));
    write(
        cx,
        &f,
        "notes/gizmo.md",
        "---\ntitle: Gizmo\n---\n",
        5,
        note(None, "gizmo"),
    );
    cx.run_until_parked();
    assert_eq!(rows(cx, &f), ["Gizmo", &create_row("gizm")]);
}

#[gpui_kit::test]
fn tab_with_nothing_to_pick_closes_it(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[   ");
    assert!(has_label(cx, &f, "No matching Items"));
    press(cx, &f, "tab");
    assert!(!picker_open(cx, &f));
}

fn create_row(query: &str) -> String {
    format!("Create Note \"{query}\"")
}

fn names_in(dir: PathBuf) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Opens `item` with the caret on a new empty line at the end of its body.
fn open_with_caret_on_new_line(cx: &mut TestAppContext, f: &Fixture, item: ItemId) {
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.open_item(item, cx)));
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_create_row_is_last_and_follows_the_query(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    // Partial matches come first, the create row last.
    input(cx, &f, "[[qu");
    assert_eq!(
        rows(cx, &f),
        [
            "Quick start",
            "Quotes",
            "Ideas on quotes",
            &create_row("qu")
        ]
    );
    // An exact title match, ignoring case, hides it.
    input(cx, &f, "otes");
    assert_eq!(rows(cx, &f), ["Quotes", "Ideas on quotes"]);
    // With nothing matching it replaces "No matching Items".
    input(cx, &f, "zz");
    assert_eq!(rows(cx, &f), [create_row("quoteszz")]);
    assert!(!has_label(cx, &f, "No matching Items"));
}

#[gpui_kit::test]
fn a_blank_query_offers_no_create_row(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[");
    assert!(!rows(cx, &f).iter().any(|r| r.starts_with("Create Note")));
}

#[gpui_kit::test]
fn creating_from_a_space_level_note_makes_a_space_level_note(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "see [[Budget 2027");
    press(cx, &f, "enter");
    assert!(!picker_open(cx, &f));
    let text = fs::read_to_string(f.data_root.join("personal/notes/budget-2027.md")).unwrap();
    assert!(text.contains("title: Budget 2027"), "{text}");
    assert_eq!(saved_body(cx, &path), "First line\n\nsee [[budget-2027]]\n");
}

#[gpui_kit::test]
fn creating_from_a_task_makes_a_note_in_its_project(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    open_with_caret_on_new_line(
        cx,
        &f,
        ItemId::Task {
            project: "kitchen-reno".into(),
            slug: "measure".into(),
        },
    );
    input(cx, &f, "[[Paint colours");
    press(cx, &f, "enter");
    let note = f
        .data_root
        .join("personal/projects/kitchen-reno/notes/paint-colours.md");
    let text = fs::read_to_string(note).unwrap();
    assert!(text.contains("title: Paint colours"), "{text}");
    assert!(!f.data_root.join("personal/notes/paint-colours.md").exists());
    // The Task stays open and holds the Link.
    let task = f
        .data_root
        .join("personal/projects/kitchen-reno/tasks/measure.md");
    assert!(saved_body(cx, &task).contains("[[paint-colours]]"));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("task-view").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn creating_from_a_journal_entry_makes_a_space_level_note(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    let date = sett_store::Date::parse("2026-11-02").unwrap();
    write(
        cx,
        &f,
        "journal/2026-11-02.md",
        "Today\n",
        900,
        Change::Journal(date),
    );
    open_with_caret_on_new_line(cx, &f, ItemId::Journal(date));
    input(cx, &f, "[[Paint colours");
    press(cx, &f, "enter");
    assert!(f.data_root.join("personal/notes/paint-colours.md").exists());
    let entry = f.data_root.join("personal/journal/2026-11-02.md");
    assert!(saved_body(cx, &entry).contains("[[paint-colours]]"));
}

#[gpui_kit::test]
fn one_undo_removes_only_the_link_and_the_note_stays(cx: &mut TestAppContext) {
    let (f, path) = on_a_new_line(cx);
    input(cx, &f, "[[Paint colours");
    press(cx, &f, "enter");
    press(cx, &f, "secondary-z");
    assert_eq!(saved_body(cx, &path), "First line\n\n\\[\\[Paint colours\n");
    assert!(names_in(f.data_root.join("personal/notes")).contains(&"paint-colours.md".to_string()));
}

#[gpui_kit::test]
fn the_current_item_stays_open_and_the_chip_row_lists_the_new_note(cx: &mut TestAppContext) {
    let (f, _) = on_a_new_line(cx);
    input(cx, &f, "[[Paint colours");
    press(cx, &f, "enter");
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("note-title").value(), Some("Plan"));
        assert!(
            window
                .within("outgoing-links")
                .try_find_by_label("Paint colours")
                .is_some()
        );
    })
    .unwrap();
}

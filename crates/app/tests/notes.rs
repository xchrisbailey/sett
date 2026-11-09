//! UI integration tests for the Notes list, the Note editor and the Journal:
//! saving only after a real edit (ADR 0003), autosave, outside changes and
//! the raw/block toggle.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::{Fixture, center_title, click_label, open_shell};
use gpui_kit::test::{TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{AppContext as _, TestAppContext};
use sett::Selection;
use sett_store::{Change, EditorMode, Settings};

const JOURNAL: &str = "0-0";
const WEBSITE: &str = "1-1";

/// Longer than the autosave delay.
const AFTER_AUTOSAVE: Duration = Duration::from_secs(2);

/// Markdown the block editor would normalise on save (`__bold__`, a setext
/// heading), behind a YAML comment that never survives a rewrite by sett.
const HAND_WRITTEN: &str =
    "---\n# by hand\ntitle: Plan\n---\nHeading\n=======\n\nSome __bold__ text\n";

fn snapshot(path: &Path) -> (Vec<u8>, SystemTime) {
    (
        fs::read(path).unwrap(),
        fs::metadata(path).unwrap().modified().unwrap(),
    )
}

/// Writes a Space-level Note by hand and shows the Notes list.
fn with_note(cx: &mut TestAppContext, f: &Fixture, slug: &str, text: &str) -> PathBuf {
    let path = f.data_root.join(format!("personal/notes/{slug}.md"));
    fs::write(&path, text).unwrap();
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(note_change(slug), cx);
            w.select(Selection::Notes, cx);
        })
    });
    path
}

fn note_change(slug: &str) -> Change {
    Change::Note {
        project: None,
        slug: slug.to_string(),
    }
}

fn wait_past_autosave(cx: &mut TestAppContext) {
    cx.executor().advance_clock(AFTER_AUTOSAVE);
    cx.run_until_parked();
}

#[gpui_kit::test]
fn opening_a_note_without_editing_never_writes_it(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", HAND_WRITTEN);
    let before = snapshot(&path);

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        // Flipping between block and raw view isn't an edit either.
        window.press("cmd-/", cx);
        window.press("cmd-/", cx);
    })
    .unwrap();
    wait_past_autosave(cx);
    cx.update_window(f.window, |_, window, cx| {
        window.within("nav").click(JOURNAL, cx);
    })
    .unwrap();
    wait_past_autosave(cx);

    assert_eq!(snapshot(&path), before);
}

#[gpui_kit::test]
fn an_edit_saves_shortly_after_typing_stops(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(
        cx,
        &f,
        "plan",
        "---\ntitle: Plan\ntags: [a]\n---\nFirst line\n",
    );

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
        window.input(" more", cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert!(!fs::read_to_string(&path).unwrap().contains("more"));

    wait_past_autosave(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("First line more"), "{text}");
    // Frontmatter the editor never sees is kept.
    assert!(
        text.contains("title: Plan") && text.contains("tags:"),
        "{text}"
    );
}

#[gpui_kit::test]
fn switching_away_saves_pending_edits_at_once(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", "---\ntitle: Plan\n---\nFirst\n");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-plan", cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
        window.input("!", cx);
        window.within("nav").click(JOURNAL, cx);
    })
    .unwrap();
    cx.run_until_parked();

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("First!"), "{text}");
}

/// Clicks into the open Item's body and types at its end.
fn type_at_end(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-body", cx);
        window.press("secondary-down", cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn open_note(cx: &mut TestAppContext, f: &Fixture, slug: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("note-{slug}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Rewrites the file as an agent would and reports it as the watcher would.
fn change_outside(cx: &mut TestAppContext, f: &Fixture, path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(note_change("plan"), cx)));
    cx.run_until_parked();
}

fn has_label(cx: &mut TestAppContext, f: &Fixture, label: &str) -> bool {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find_by_label(label).is_some()
    })
    .unwrap()
}

fn click(cx: &mut TestAppContext, f: &Fixture, label: &str) {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        click_label(window, None, label, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[gpui_kit::test]
fn the_journal_opens_today_and_writes_only_once_typed_in(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let journal = f.data_root.join("personal/journal");

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("nav").click(JOURNAL, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(center_title(window), "Journal");
        assert!(window.try_find("note-body").is_some());
    })
    .unwrap();
    wait_past_autosave(cx);
    assert_eq!(fs::read_dir(&journal).unwrap().count(), 0);

    type_at_end(cx, &f, "Woke early");
    wait_past_autosave(cx);
    let text = fs::read_to_string(journal.join(format!("{}.md", today()))).unwrap();
    assert_eq!(text.trim_end(), "Woke early");
}

#[gpui_kit::test]
fn an_outside_change_reloads_a_note_without_edits(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", "---\ntitle: Plan\n---\nMine\n");
    open_note(cx, &f, "plan");

    change_outside(cx, &f, &path, "---\ntitle: Plan\n---\nTheirs\n");
    assert!(!has_label(cx, &f, "Keep mine"));
    wait_past_autosave(cx);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "---\ntitle: Plan\n---\nTheirs\n"
    );

    // The editor now holds their text.
    type_at_end(cx, &f, "!");
    wait_past_autosave(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("Theirs!") && !text.contains("Mine"), "{text}");
}

#[gpui_kit::test]
fn an_outside_change_during_edits_asks_and_can_keep_mine(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", "---\ntitle: Plan\n---\nFirst\n");
    open_note(cx, &f, "plan");
    type_at_end(cx, &f, " mine");

    change_outside(cx, &f, &path, "---\ntitle: Plan\n---\nTheirs\n");
    assert!(has_label(cx, &f, "Keep mine"));
    // Nothing is saved while the question is open.
    wait_past_autosave(cx);
    assert!(fs::read_to_string(&path).unwrap().contains("Theirs"));

    click(cx, &f, "Keep mine");
    wait_past_autosave(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("First mine") && !text.contains("Theirs"),
        "{text}"
    );
}

#[gpui_kit::test]
fn an_outside_change_during_edits_can_load_theirs(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", "---\ntitle: Plan\n---\nFirst\n");
    open_note(cx, &f, "plan");
    type_at_end(cx, &f, " mine");

    let theirs = "---\n# agent\ntitle: Plan\n---\nTheirs\n";
    change_outside(cx, &f, &path, theirs);
    click(cx, &f, "Load theirs");
    wait_past_autosave(cx);
    assert_eq!(fs::read_to_string(&path).unwrap(), theirs);
}

#[gpui_kit::test]
fn the_raw_toggle_is_global_and_remembered(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let path = with_note(cx, &f, "plan", HAND_WRITTEN);
    let mode =
        |f: &Fixture| Settings::load(&f.settings_path, Path::new("/nonexistent")).editor_mode;
    open_note(cx, &f, "plan");
    assert!(has_label(cx, &f, "Block editor"));

    cx.update_window(f.window, |_, window, cx| window.press("cmd-/", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(mode(&f), EditorMode::Raw);
    assert!(has_label(cx, &f, "Raw Markdown"));

    // Raw edits keep the Markdown exactly as written.
    type_at_end(cx, &f, "More");
    wait_past_autosave(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.ends_with("Heading\n=======\n\nSome __bold__ text\nMore"),
        "{text}"
    );

    // The Journal follows the same mode.
    cx.update_window(f.window, |_, window, cx| {
        window.within("nav").click(JOURNAL, cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(has_label(cx, &f, "Raw Markdown"));

    cx.update_window(f.window, |_, window, cx| window.press("cmd-/", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(mode(&f), EditorMode::Block);
    assert!(has_label(cx, &f, "Block editor"));
}

fn set_mtime(path: &Path, secs_ago: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(secs_ago))
        .unwrap();
}

fn listed(cx: &mut TestAppContext, f: &Fixture) -> Vec<String> {
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window
            .within("notes-list")
            .find_all_by_role(gpui_kit::Role::ListItem)
            .iter()
            .filter_map(|n| n.label().map(str::to_string))
            .collect()
    })
    .unwrap()
}

#[gpui_kit::test]
fn notes_are_listed_most_recently_edited_first(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let older = f.data_root.join("personal/notes/older.md");
    fs::write(&older, "---\ntitle: Older\n---\nOld\n").unwrap();
    set_mtime(&older, 3600);
    with_note(cx, &f, "newer", "---\ntitle: Newer\n---\nNew\n");
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.apply_change(note_change("older"), cx)));
    assert_eq!(listed(cx, &f), ["Newer", "Older"]);

    open_note(cx, &f, "older");
    type_at_end(cx, &f, "!");
    wait_past_autosave(cx);
    assert_eq!(listed(cx, &f), ["Older", "Newer"]);
}

#[gpui_kit::test]
fn a_new_note_starts_untitled_and_keeps_its_slug_when_renamed(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Notes, cx)));

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-note", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let path = f.data_root.join("personal/notes/untitled.md");
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("title: Untitled")
    );

    // The title is focused and selected, so typing replaces it.
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.input("Groceries", cx);
    })
    .unwrap();
    wait_past_autosave(cx);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("title: Groceries"), "{text}");
    assert!(!f.data_root.join("personal/notes/groceries.md").exists());
    assert_eq!(listed(cx, &f), ["Groceries"]);
}

#[gpui_kit::test]
fn deleting_a_note_confirms_then_trashes_it(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    let model = f.model(cx);
    // Stands in for the macOS Trash, which these tests must not touch.
    cx.update(|cx| model.update(cx, |w, _| w.set_trash(|path| fs::remove_file(path))));
    let path = with_note(cx, &f, "plan", "---\ntitle: Plan\n---\nBody\n");
    open_note(cx, &f, "plan");

    cx.update_window(f.window, |_, window, cx| {
        window.right_click("note-plan", cx);
    })
    .unwrap();
    cx.run_until_parked();
    click(cx, &f, "Delete\u{2026}");
    click(cx, &f, "Cancel");
    assert!(path.exists());

    cx.update_window(f.window, |_, window, cx| {
        window.right_click("note-plan", cx);
    })
    .unwrap();
    cx.run_until_parked();
    click(cx, &f, "Delete\u{2026}");
    click(cx, &f, "Delete");
    wait_past_autosave(cx);

    assert!(!path.exists());
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("note-plan").is_none());
        assert!(window.try_find("note-body").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_projects_notes_tab_lists_only_its_notes(cx: &mut TestAppContext) {
    let f = open_shell(cx);
    with_note(cx, &f, "plan", "---\ntitle: Plan\n---\n");
    let model = f.model(cx);
    cx.update(|cx| {
        model.update(cx, |w, cx| w.create_project("Website", None, cx).unwrap());
    });
    let notes = f.data_root.join("personal/projects/website/notes");
    fs::write(notes.join("spec.md"), "---\ntitle: Spec\n---\n").unwrap();
    cx.update(|cx| {
        model.update(cx, |w, cx| {
            w.apply_change(Change::Project("website".into()), cx)
        })
    });

    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.within("nav").click(WEBSITE, cx);
        click_label(window, Some("project-tabs"), "Notes", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(listed(cx, &f), ["Spec"]);

    cx.update_window(f.window, |_, window, cx| {
        window.click("new-note", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(notes.join("untitled.md").exists());
    assert!(!f.data_root.join("personal/notes/untitled.md").exists());
}

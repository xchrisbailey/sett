use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use sett_store::{Change, Date, JournalEntry, Space, Store};

/// FSEvents latency varies, so wait up to a bound rather than a fixed sleep.
const TIMEOUT: Duration = Duration::from_secs(5);

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    (dir, store)
}

/// Collects events until every one of `wanted` has arrived, returning all of
/// them. Panics after `TIMEOUT`.
fn collect_until(rx: &Receiver<Change>, wanted: &[Change]) -> Vec<Change> {
    let deadline = Instant::now() + TIMEOUT;
    let mut seen = Vec::new();
    while !wanted.iter().all(|w| seen.contains(w)) {
        match rx.try_recv() {
            Ok(change) => seen.push(change),
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(_) => panic!("timed out waiting for {wanted:?}; saw {seen:?}"),
        }
    }
    seen
}

fn task(project: &str, slug: &str) -> Change {
    Change::Task {
        project: project.into(),
        slug: slug.into(),
    }
}

#[test]
fn outside_changes_are_reported_quickly() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let watcher = store.watch(Space::Work).unwrap();
    let rx = watcher.events();

    let started = Instant::now();
    write(
        &root,
        "work/projects/inbox/tasks/a.md",
        "---\ntitle: A\n---\n",
    );
    collect_until(&rx, &[task("inbox", "a")]);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "took {:?}",
        started.elapsed()
    );

    write(
        &root,
        "work/projects/inbox/tasks/a.md",
        "---\ntitle: A2\n---\n",
    );
    collect_until(&rx, &[task("inbox", "a")]);

    fs::rename(
        root.join("work/projects/inbox/tasks/a.md"),
        root.join("work/projects/inbox/tasks/b.md"),
    )
    .unwrap();
    collect_until(&rx, &[task("inbox", "a"), task("inbox", "b")]);

    fs::remove_file(root.join("work/projects/inbox/tasks/b.md")).unwrap();
    collect_until(&rx, &[task("inbox", "b")]);

    write(&root, "work/journal/2026-03-04.md", "Hi\n");
    write(&root, "work/notes/idea.md", "Idea\n");
    collect_until(
        &rx,
        &[
            Change::Journal(Date::parse("2026-03-04").unwrap()),
            Change::Note {
                project: None,
                slug: "idea".into(),
            },
        ],
    );

    fs::rename(
        root.join("work/projects/inbox"),
        root.join("work/projects/renamed"),
    )
    .unwrap();
    collect_until(
        &rx,
        &[
            Change::Project("inbox".into()),
            Change::Project("renamed".into()),
        ],
    );
}

#[test]
fn own_writes_and_other_space_are_ignored() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let today = Date::parse("2026-03-04").unwrap();
    let watcher = store.watch(Space::Work).unwrap();
    let rx = watcher.events();

    let mut t = store
        .create_task(Space::Work, "inbox", "Own task", today)
        .unwrap();
    t.title = "Renamed by sett".into();
    store.save_task(&mut t).unwrap();
    let mut project = store.create_project(Space::Work, "Own project").unwrap();
    project.name = "Still mine".into();
    store.save_project(&mut project).unwrap();
    store
        .create_note(Space::Work, Some(&project.slug), "Own note", today)
        .unwrap();
    store
        .save_journal_entry(&mut JournalEntry::new(Space::Work, today))
        .unwrap();
    let doomed = store
        .create_task(Space::Work, "inbox", "Doomed", today)
        .unwrap();
    store.trash_task(&doomed).unwrap();
    store.trash_project(&project).unwrap();

    write(&root, "personal/notes/private.md", "Not for work\n");

    // Events arrive in order, so once the sentinel shows up everything before
    // it has been delivered or dropped.
    let sentinel = Change::Note {
        project: None,
        slug: "sentinel".into(),
    };
    write(&root, "work/notes/sentinel.md", "Outside\n");
    let seen = collect_until(&rx, std::slice::from_ref(&sentinel));
    assert_eq!(seen, [sentinel]);
}

#[test]
fn outside_edit_after_own_write_is_still_reported() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let today = Date::parse("2026-03-04").unwrap();
    let watcher = store.watch(Space::Work).unwrap();
    let rx = watcher.events();

    let t = store
        .create_task(Space::Work, "inbox", "Shared", today)
        .unwrap();
    write(
        &root,
        &format!("work/projects/inbox/tasks/{}.md", t.slug),
        "---\ntitle: Agent edit\n---\n",
    );
    collect_until(&rx, &[task("inbox", &t.slug)]);
}

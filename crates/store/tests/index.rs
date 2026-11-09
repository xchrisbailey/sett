use std::fs;
use std::path::Path;

use sett_store::{Change, Date, Space, SpaceIndex, Store, Update};

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

fn task(project: &str, slug: &str) -> Change {
    Change::Task {
        project: project.into(),
        slug: slug.into(),
    }
}

#[test]
fn scan_covers_one_space_only() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(
        &root,
        "work/projects/inbox/tasks/a.md",
        "---\ntitle: A\n---\n",
    );
    write(&root, "personal/notes/n.md", "---\ntitle: N\n---\n");
    write(&root, "personal/journal/2026-03-04.md", "Hello\n");

    let work = SpaceIndex::scan(&store, Space::Work);
    assert_eq!(work.space(), Space::Work);
    assert_eq!(work.tasks("inbox").len(), 1);
    assert!(work.notes(None).is_empty());
    assert!(work.journal().is_empty());

    let personal = SpaceIndex::scan(&store, Space::Personal);
    assert!(personal.tasks("inbox").is_empty());
    assert_eq!(personal.notes(None)[0].title, "N");
    assert_eq!(personal.journal()[0].body, "Hello\n");
}

#[test]
fn apply_reloads_what_is_on_disk_now() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let mut index = SpaceIndex::scan(&store, Space::Work);

    write(
        &root,
        "work/projects/inbox/tasks/a.md",
        "---\ntitle: A\n---\n",
    );
    assert_eq!(index.apply(&store, &task("inbox", "a")), Update::Upserted);
    assert_eq!(index.task("inbox", "a").unwrap().title, "A");

    // A repeated event for the same content is a no-op, so callers don't
    // reload or prompt twice.
    assert_eq!(index.apply(&store, &task("inbox", "a")), Update::Unchanged);

    write(
        &root,
        "work/projects/inbox/tasks/a.md",
        "---\ntitle: A2\n---\n",
    );
    assert_eq!(index.apply(&store, &task("inbox", "a")), Update::Upserted);
    assert_eq!(index.task("inbox", "a").unwrap().title, "A2");

    // A rename arrives as two changes: the old name is gone, the new one appears.
    fs::rename(
        root.join("work/projects/inbox/tasks/a.md"),
        root.join("work/projects/inbox/tasks/b.md"),
    )
    .unwrap();
    assert_eq!(index.apply(&store, &task("inbox", "a")), Update::Removed);
    assert_eq!(index.apply(&store, &task("inbox", "b")), Update::Upserted);
    assert!(index.task("inbox", "a").is_none());
    assert_eq!(index.tasks("inbox")[0].slug, "b");

    write(&root, "work/notes/n.md", "---\ntitle: N\n---\n");
    let note = Change::Note {
        project: None,
        slug: "n".into(),
    };
    assert_eq!(index.apply(&store, &note), Update::Upserted);
    assert_eq!(index.note(None, "n").unwrap().title, "N");

    let day = Date::parse("2026-03-04").unwrap();
    write(&root, "work/journal/2026-03-04.md", "Hi\n");
    assert_eq!(index.apply(&store, &Change::Journal(day)), Update::Upserted);
    fs::remove_file(root.join("work/journal/2026-03-04.md")).unwrap();
    assert_eq!(index.apply(&store, &Change::Journal(day)), Update::Removed);
    assert!(index.journal_entry(day).is_none());
}

#[test]
fn project_change_reloads_the_whole_project_folder() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(
        &root,
        "work/projects/api/project.md",
        "---\nname: API\n---\n",
    );
    write(
        &root,
        "work/projects/api/tasks/t.md",
        "---\ntitle: T\n---\n",
    );
    write(
        &root,
        "work/projects/api/notes/n.md",
        "---\ntitle: N\n---\n",
    );
    let mut index = SpaceIndex::scan(&store, Space::Work);
    assert_eq!(index.projects().len(), 2);

    // Moving a folder only reports the folder itself, so the Project change
    // has to carry its Tasks and Notes with it.
    fs::rename(
        root.join("work/projects/api"),
        root.join("work/projects/backend"),
    )
    .unwrap();
    let api = Change::Project("api".into());
    let backend = Change::Project("backend".into());
    assert_eq!(index.apply(&store, &api), Update::Removed);
    assert!(index.tasks("api").is_empty());
    assert!(index.note(Some("api"), "n").is_none());
    assert_eq!(index.apply(&store, &backend), Update::Upserted);
    assert_eq!(index.tasks("backend")[0].title, "T");
    assert_eq!(index.note(Some("backend"), "n").unwrap().title, "N");
    let slugs: Vec<&str> = index.projects().iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs, ["inbox", "backend"]);

    fs::remove_dir_all(root.join("work/projects/backend")).unwrap();
    assert_eq!(index.apply(&store, &Change::All), Update::Upserted);
    assert_eq!(index.projects().len(), 1);
    assert!(index.tasks("backend").is_empty());
}

#[test]
fn open_item_knows_when_a_change_covers_it() {
    let open = task("api", "t");
    assert!(task("api", "t").covers(&open));
    assert!(Change::Project("api".into()).covers(&open));
    assert!(Change::All.covers(&open));
    assert!(!task("api", "other").covers(&open));
    assert!(!Change::Project("web".into()).covers(&open));

    let note = Change::Note {
        project: Some("api".into()),
        slug: "n".into(),
    };
    assert!(Change::Project("api".into()).covers(&note));
    let space_note = Change::Note {
        project: None,
        slug: "n".into(),
    };
    assert!(!Change::Project("api".into()).covers(&space_note));
}

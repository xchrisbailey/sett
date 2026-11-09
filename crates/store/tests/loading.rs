use std::fs;
use std::path::Path;

use sett_store::{Date, Space, Store};

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn date(s: &str) -> Date {
    Date::parse(s).unwrap()
}

/// A data root that already exists, so first-run setup is skipped.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "work/projects/api/project.md",
        "---\nname: API\nicon: server\ncolumns: [Backlog, Doing, Review, Shipped]\narchived: true\n---\nProject notes\n",
    );
    write(
        root,
        "work/projects/api/tasks/add-auth.md",
        "---\ntitle: Add auth\ncolumn: Review\norder: 2.5\ncreated: 2026-01-03\ndue: 2026-02-01\nowner: sam\n---\n# Plan\n\nSee [[api-design]].\n",
    );
    write(
        root,
        "work/projects/api/tasks/ship-it.md",
        "---\ntitle: Ship it\ncolumn: Shipped\norder: 1\ncreated: 2026-01-01\ndone: 2026-01-09\n---\n",
    );
    dir
}

#[test]
fn loads_project_and_task_frontmatter() {
    let dir = fixture();
    let store = Store::open(dir.path()).unwrap();

    let api = store
        .projects(Space::Work)
        .into_iter()
        .find(|p| p.slug == "api")
        .unwrap();
    assert_eq!(api.name, "API");
    assert_eq!(api.icon.as_deref(), Some("server"));
    assert_eq!(api.columns, ["Backlog", "Doing", "Review", "Shipped"]);
    assert!(api.archived);

    let tasks = store.tasks(Space::Work, "api");
    let slugs: Vec<_> = tasks.iter().map(|t| t.slug.as_str()).collect();
    assert_eq!(slugs, ["ship-it", "add-auth"], "sorted by order");

    let auth = &tasks[1];
    assert_eq!(auth.title, "Add auth");
    assert_eq!(auth.column.as_deref(), Some("Review"));
    assert_eq!(auth.order, Some(2.5));
    assert_eq!(auth.created, Some(date("2026-01-03")));
    assert_eq!(auth.due, Some(date("2026-02-01")));
    assert_eq!(auth.done, None);
    assert_eq!(auth.body, "# Plan\n\nSee [[api-design]].\n");
    assert_eq!(tasks[0].done, Some(date("2026-01-09")));
}

#[test]
fn malformed_or_missing_frontmatter_loads_with_defaults_and_is_not_rewritten() {
    let dir = fixture();
    let root = dir.path();
    let files = [
        ("no-frontmatter.md", "Just a body\n"),
        ("broken-yaml.md", "---\ntitle: [unclosed\n---\nbody\n"),
        ("unterminated.md", "---\ntitle: Never closed\nbody\n"),
        (
            "bad-values.md",
            "---\ntitle: Bad values\ncolumn: [1, 2]\norder: soon\ncreated: 2026-02-30\ndue: tomorrow\n---\n",
        ),
        ("not-a-map.md", "---\n- a\n- b\n---\nlist frontmatter\n"),
    ];
    for (name, text) in files {
        write(root, &format!("work/projects/api/tasks/{name}"), text);
    }
    // Not Items: must be skipped, not crash the load.
    write(root, "work/projects/api/tasks/.DS_Store", "junk");
    write(root, "work/projects/api/tasks/image.png", "junk");
    fs::write(
        root.join("work/projects/api/tasks/binary.md"),
        [0xff, 0xfe, 0x00],
    )
    .unwrap();

    let store = Store::open(root).unwrap();
    let tasks = store.tasks(Space::Work, "api");
    let get = |slug: &str| tasks.iter().find(|t| t.slug == slug).unwrap();

    let plain = get("no-frontmatter");
    assert_eq!(plain.title, "no-frontmatter", "title falls back to slug");
    assert_eq!(plain.column, None);
    assert_eq!(plain.body, "Just a body\n");

    assert_eq!(get("broken-yaml").title, "broken-yaml");
    assert_eq!(get("unterminated").body, "---\ntitle: Never closed\nbody\n");
    assert_eq!(get("not-a-map").body, "list frontmatter\n");

    let bad = get("bad-values");
    assert_eq!(bad.title, "Bad values");
    assert_eq!(bad.column, None);
    assert_eq!(bad.order, None);
    assert_eq!(bad.created, None, "2026-02-30 is not a real date");
    assert_eq!(bad.due, None);

    assert_eq!(tasks.len(), 7);
    for (name, text) in files {
        let on_disk =
            fs::read_to_string(root.join(format!("work/projects/api/tasks/{name}"))).unwrap();
        assert_eq!(on_disk, text, "{name} must not be fixed on load");
    }
}

#[test]
fn project_without_project_md_still_loads() {
    let dir = fixture();
    fs::create_dir_all(dir.path().join("personal/projects/garden/tasks")).unwrap();
    let store = Store::open(dir.path()).unwrap();

    let garden = &store.projects(Space::Personal)[0];
    assert_eq!(garden.slug, "garden");
    assert_eq!(garden.name, "garden");
    assert_eq!(garden.columns, ["Todo", "Doing", "Done"]);
    assert!(
        !dir.path()
            .join("personal/projects/garden/project.md")
            .exists()
    );
}

#[test]
fn loads_notes_and_journal_entries() {
    let dir = fixture();
    let root = dir.path();
    write(
        root,
        "personal/notes/books.md",
        "---\ntitle: Books\ncreated: 2026-01-02\n---\nDune\n",
    );
    write(
        root,
        "work/projects/api/notes/api-design.md",
        "---\ntitle: API design\n---\nREST\n",
    );
    write(root, "personal/journal/2026-01-05.md", "Cold today\n");
    write(
        root,
        "personal/journal/2026-01-04.md",
        "---\nmood: good\n---\nWarm\n",
    );
    write(root, "personal/journal/not-a-date.md", "ignored\n");

    let store = Store::open(root).unwrap();

    let notes = store.notes(Space::Personal, None);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "Books");
    assert_eq!(notes[0].created, Some(date("2026-01-02")));
    assert_eq!(notes[0].body, "Dune\n");

    let project_notes = store.notes(Space::Work, Some("api"));
    assert_eq!(project_notes[0].slug, "api-design");
    assert!(
        store.notes(Space::Work, None).is_empty(),
        "Spaces are isolated"
    );

    let journal = store.journal(Space::Personal);
    let dates: Vec<_> = journal.iter().map(|e| e.date).collect();
    assert_eq!(dates, [date("2026-01-04"), date("2026-01-05")]);
    assert_eq!(journal[0].body, "Warm\n");
    assert!(store.journal(Space::Work).is_empty());
}

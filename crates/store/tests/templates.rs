use std::fs;

use sett_store::{Date, ItemId, LinkTarget, Space, SpaceIndex, Store};

fn new_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().join("sett")).unwrap();
    store.set_trash(|path| fs::remove_file(path));
    (dir, store)
}

#[test]
fn create_makes_the_folder_and_numbers_clashes_within_templates_only() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let today = Date::parse("2026-03-04").unwrap();
    assert!(!root.join("work/templates").exists());

    // A Note with the same slug doesn't count toward the Template's slug.
    store
        .create_note(Space::Work, None, "Bug report", today)
        .unwrap();
    let first = store.create_template(Space::Work, "Bug report").unwrap();
    let second = store.create_template(Space::Work, "bug report").unwrap();
    assert_eq!(first.slug, "bug-report");
    assert_eq!(second.slug, "bug-report-2");
    assert!(root.join("work/templates/bug-report.md").is_file());

    // The Note created afterwards isn't pushed to a new slug by the Template.
    let note = store
        .create_note(Space::Work, None, "Bug report", today)
        .unwrap();
    assert_eq!(note.slug, "bug-report-2");
}

#[test]
fn save_keeps_unknown_keys_and_the_body_holds_no_frontmatter() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    fs::create_dir_all(root.join("work/templates")).unwrap();
    fs::write(
        root.join("work/templates/standup.md"),
        "---\ntitle: Standup\ncolumn: Doing\nowner: me\n---\n## Notes\n",
    )
    .unwrap();

    let mut t = store.template(Space::Work, "standup").unwrap();
    assert_eq!(t.title, "Standup");
    assert_eq!(t.body, "## Notes\n");
    t.body = "## Agenda\n".into();
    store.save_template(&mut t).unwrap();

    let text = fs::read_to_string(root.join("work/templates/standup.md")).unwrap();
    assert_eq!(
        text,
        "---\ntitle: Standup\ncolumn: Doing\nowner: me\n---\n## Agenda\n"
    );
}

#[test]
fn saving_a_template_with_broken_frontmatter_leaves_the_file_alone() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    fs::create_dir_all(root.join("work/templates")).unwrap();
    let raw = "---\ntitle: [oops\n---\nBody\n";
    fs::write(root.join("work/templates/bad.md"), raw).unwrap();

    let mut t = store.template(Space::Work, "bad").unwrap();
    t.body = "Changed".into();
    assert!(store.save_template(&mut t).is_err());
    assert_eq!(
        fs::read_to_string(root.join("work/templates/bad.md")).unwrap(),
        raw
    );
}

#[test]
fn templates_are_listed_by_title() {
    let (_dir, store) = new_store();
    for title in ["Zebra", "apple", "Mango"] {
        store.create_template(Space::Work, title).unwrap();
    }
    store.create_template(Space::Personal, "Other").unwrap();
    let titles: Vec<String> = store
        .templates(Space::Work)
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["apple", "Mango", "Zebra"]);
}

#[test]
fn rename_moves_to_a_fresh_slug_and_is_a_no_op_when_it_would_not_change() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let a = store.create_template(Space::Work, "Alpha").unwrap();
    store.create_template(Space::Work, "Beta").unwrap();

    // Same title: no-op.
    assert_eq!(
        store
            .rename_template(Space::Work, &a.slug, "Alpha")
            .unwrap(),
        "alpha"
    );
    assert!(root.join("work/templates/alpha.md").is_file());

    // A clash gets a suffix.
    assert_eq!(
        store.rename_template(Space::Work, "alpha", "Beta").unwrap(),
        "beta-2"
    );
    assert!(!root.join("work/templates/alpha.md").exists());
    assert!(root.join("work/templates/beta-2.md").is_file());

    // Its own slug doesn't count as a clash.
    assert_eq!(
        store
            .rename_template(Space::Work, "beta-2", "Beta 2")
            .unwrap(),
        "beta-2"
    );

    // A plain rename.
    assert_eq!(
        store
            .rename_template(Space::Work, "beta-2", "Gamma")
            .unwrap(),
        "gamma"
    );
    assert!(store.template(Space::Work, "gamma").is_some());
    assert!(store.rename_template(Space::Work, "missing", "X").is_err());
}

#[test]
fn delete_removes_the_file() {
    let (_dir, store) = new_store();
    let t = store.create_template(Space::Work, "Gone").unwrap();
    store.trash_template(&t).unwrap();
    assert!(store.template(Space::Work, "gone").is_none());
    assert!(store.templates(Space::Work).is_empty());
}

#[test]
fn a_template_is_never_an_item() {
    let (_dir, store) = new_store();
    let mut t = store.create_template(Space::Work, "Meeting").unwrap();
    t.body = "[[meeting]]\n".into();
    store.save_template(&mut t).unwrap();

    assert!(store.notes(Space::Work, None).is_empty());
    let index = SpaceIndex::scan(&store, Space::Work);
    assert!(index.notes(None).is_empty());
    assert_eq!(index.resolve("meeting"), LinkTarget::Broken);
    assert!(
        index
            .links(&ItemId::Note {
                project: None,
                slug: "meeting".into()
            })
            .is_empty()
    );
}

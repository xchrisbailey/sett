use std::fs;
use std::path::Path;

use sett_store::{Backlink, Change, ItemId, Link, LinkTarget, Space, SpaceIndex, Store};

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

fn space_note(slug: &str) -> ItemId {
    ItemId::Note {
        project: None,
        slug: slug.into(),
    }
}

#[test]
fn backlinks_list_the_items_that_link_here() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(&root, "work/notes/plan.md", "---\ntitle: Plan\n---\n");
    write(
        &root,
        "work/notes/ideas.md",
        "See [[plan]] and [[plan|the plan]].\n",
    );
    write(
        &root,
        "work/projects/inbox/tasks/ship.md",
        "---\ntitle: Ship\n---\nFollow [[plan#Steps]]\n",
    );
    let index = SpaceIndex::scan(&store, Space::Work);

    assert_eq!(
        index.backlinks(&space_note("plan")),
        vec![
            Backlink {
                source: space_note("ideas"),
                ambiguous: false
            },
            Backlink {
                source: ItemId::Task {
                    project: "inbox".into(),
                    slug: "ship".into()
                },
                ambiguous: false
            },
        ]
    );
    assert!(index.backlinks(&space_note("ideas")).is_empty());
}

fn task_id(project: &str, slug: &str) -> ItemId {
    ItemId::Task {
        project: project.into(),
        slug: slug.into(),
    }
}

fn slugs(links: &[Link]) -> Vec<&str> {
    links.iter().map(|l| l.slug.as_str()).collect()
}

#[test]
fn links_in_code_are_not_links() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(
        &root,
        "work/notes/doc.md",
        "Real [[a]], code `[[b]]` and ``x ` [[c]]``.\n\
         Unclosed ` tick then [[d]].\n\
         ```md\n[[e]]\n```\n\
         ~~~~\n```\n[[f]]\n~~~~\n\
         [[a]] again, [[ g | label ]], [[|x]], [[]] and [[h\n",
    );
    let index = SpaceIndex::scan(&store, Space::Work);

    assert_eq!(slugs(&index.links(&space_note("doc"))), vec!["a", "d", "g"]);
}

#[test]
fn unresolved_links_are_broken() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(
        &root,
        "work/notes/doc.md",
        "[[nowhere]] and [[2026-11-09]]\n",
    );
    let index = SpaceIndex::scan(&store, Space::Work);

    let links = index.links(&space_note("doc"));
    assert!(links.iter().all(|l| l.target == LinkTarget::Broken));
}

#[test]
fn journal_entries_are_linked_by_date() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(&root, "work/journal/2026-11-09.md", "Met about [[plan]]\n");
    write(&root, "work/notes/plan.md", "From [[2026-11-09]]\n");
    let index = SpaceIndex::scan(&store, Space::Work);
    let day = ItemId::Journal(sett_store::Date::parse("2026-11-09").unwrap());

    assert_eq!(
        index.resolve("2026-11-09"),
        LinkTarget::Found {
            item: day.clone(),
            ambiguous: false
        }
    );
    assert_eq!(
        index.backlinks(&space_note("plan")),
        vec![Backlink {
            source: day,
            ambiguous: false
        }]
    );
    assert_eq!(index.title(&space_note("plan")).as_deref(), Some("plan"));
}

/// Agents can create files sett would never name the same.
#[test]
fn duplicate_slugs_resolve_by_path_with_space_notes_first() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(&root, "work/projects/b/tasks/x.md", "");
    write(&root, "work/projects/a/tasks/x.md", "");
    write(&root, "work/projects/b/notes/x.md", "");
    write(&root, "work/notes/doc.md", "[[x]]\n");
    let mut index = SpaceIndex::scan(&store, Space::Work);

    let found = |item| LinkTarget::Found {
        item,
        ambiguous: true,
    };
    assert_eq!(index.resolve("x"), found(task_id("a", "x")));
    assert_eq!(
        index.backlinks(&task_id("a", "x")),
        vec![Backlink {
            source: space_note("doc"),
            ambiguous: true
        }]
    );
    // The losing candidates have no backlinks rather than sharing them.
    assert!(index.backlinks(&task_id("b", "x")).is_empty());

    write(&root, "work/notes/x.md", "");
    index.apply(
        &store,
        &Change::Note {
            project: None,
            slug: "x".into(),
        },
    );
    assert_eq!(index.resolve("x"), found(space_note("x")));
}

#[test]
fn the_graph_follows_changes_to_one_item() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(&root, "work/notes/plan.md", "");
    write(&root, "work/notes/doc.md", "[[plan]]\n");
    let mut index = SpaceIndex::scan(&store, Space::Work);
    let doc = Change::Note {
        project: None,
        slug: "doc".into(),
    };

    write(&root, "work/notes/doc.md", "[[other]]\n");
    index.apply(&store, &doc);
    assert!(index.backlinks(&space_note("plan")).is_empty());
    assert_eq!(slugs(&index.links(&space_note("doc"))), vec!["other"]);

    write(&root, "work/notes/doc.md", "[[plan]]\n");
    index.apply(&store, &doc);
    assert_eq!(index.backlinks(&space_note("plan")).len(), 1);

    // Removing the target breaks the Link; removing the source drops it.
    fs::remove_file(root.join("work/notes/plan.md")).unwrap();
    index.apply(
        &store,
        &Change::Note {
            project: None,
            slug: "plan".into(),
        },
    );
    assert_eq!(
        index.links(&space_note("doc"))[0].target,
        LinkTarget::Broken
    );

    write(&root, "work/projects/site/tasks/plan.md", "");
    write(&root, "work/projects/site/notes/n.md", "[[doc]]\n");
    index.apply(&store, &Change::Project("site".into()));
    assert_eq!(
        index.resolve("plan"),
        LinkTarget::Found {
            item: task_id("site", "plan"),
            ambiguous: false
        }
    );
    assert_eq!(index.backlinks(&space_note("doc")).len(), 1);

    fs::remove_dir_all(root.join("work/projects/site")).unwrap();
    index.apply(&store, &Change::Project("site".into()));
    assert_eq!(index.resolve("plan"), LinkTarget::Broken);
    assert!(index.backlinks(&space_note("doc")).is_empty());
}

#[test]
fn links_never_reach_the_other_space() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    write(&root, "personal/notes/secret.md", "[[doc]]\n");
    write(&root, "work/notes/doc.md", "[[secret]]\n");
    let index = SpaceIndex::scan(&store, Space::Work);

    assert_eq!(
        index.links(&space_note("doc"))[0].target,
        LinkTarget::Broken
    );
    assert!(index.backlinks(&space_note("doc")).is_empty());
}

#[test]
fn new_items_get_slugs_unique_across_the_space() {
    let (_dir, store) = new_store();
    let root = store.root().to_path_buf();
    let today = sett_store::Date::parse("2026-11-09").unwrap();
    write(&root, "work/notes/plan.md", "");
    write(&root, "work/projects/site/tasks/plan-2.md", "");
    write(
        &root,
        "work/projects/old/project.md",
        "---\narchived: true\n---\n",
    );
    write(&root, "work/projects/old/notes/plan-3.md", "");
    // The other Space doesn't count.
    write(&root, "personal/notes/plan-4.md", "");

    let task = store
        .create_task(Space::Work, "inbox", "Plan", today)
        .unwrap();
    assert_eq!(task.slug, "plan-4");
    let note = store
        .create_note(Space::Work, Some("inbox"), "Plan", today)
        .unwrap();
    assert_eq!(note.slug, "plan-5");

    // A date-shaped slug would collide with the Journal Entry for that day.
    let note = store
        .create_note(Space::Work, None, "2026-11-09", today)
        .unwrap();
    assert_eq!(note.slug, "2026-11-09-2");
}

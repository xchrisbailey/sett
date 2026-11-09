use std::collections::BTreeMap;

use crate::links::{Backlink, ItemId, Link, LinkGraph, LinkTarget};
use crate::{Date, JournalEntry, Note, Project, Space, Store, Task};

/// Something in a Space that may have changed on disk. Applying a `Change`
/// re-reads that thing, so the same `Change` covers create, edit and delete.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Change {
    /// The Project folder or its `project.md`. Re-reads its Tasks and Notes
    /// too, because moving a folder only reports the folder.
    Project(String),
    Task {
        project: String,
        slug: String,
    },
    Note {
        project: Option<String>,
        slug: String,
    },
    Journal(Date),
    /// A top-level folder of the Space moved or vanished: rescan everything.
    All,
}

impl Change {
    /// Whether applying `self` may have changed `item`. An open Item is
    /// identified by its own `Change`; when this returns true the caller looks
    /// it up in the index again and, if the user has unsaved edits, asks
    /// whether to keep theirs or load the version on disk.
    pub fn covers(&self, item: &Change) -> bool {
        match (self, item) {
            (Change::All, _) => true,
            (Change::Project(p), Change::Task { project, .. }) => p == project,
            (Change::Project(p), Change::Note { project, .. }) => project.as_ref() == Some(p),
            _ => self == item,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update {
    Upserted,
    Removed,
    /// Disk already matches the index, e.g. a duplicate event.
    Unchanged,
}

/// Everything in one Space, held in memory. An index never spans both Spaces
/// (ADR 0002).
#[derive(Debug, Clone)]
pub struct SpaceIndex {
    space: Space,
    projects: BTreeMap<String, Project>,
    tasks: BTreeMap<(String, String), Task>,
    notes: BTreeMap<(Option<String>, String), Note>,
    journal: BTreeMap<Date, JournalEntry>,
    links: LinkGraph,
}

impl SpaceIndex {
    pub fn scan(store: &Store, space: Space) -> SpaceIndex {
        let mut index = SpaceIndex {
            space,
            projects: BTreeMap::new(),
            tasks: BTreeMap::new(),
            notes: BTreeMap::new(),
            journal: BTreeMap::new(),
            links: LinkGraph::default(),
        };
        for note in store.notes(space, None) {
            index.link_note(&note);
            index.notes.insert((None, note.slug.clone()), note);
        }
        for entry in store.journal(space) {
            index
                .links
                .set(ItemId::Journal(entry.date), Some(&entry.body));
            index.journal.insert(entry.date, entry);
        }
        for project in store.projects(space) {
            index.load_project_items(store, &project.slug);
            index.projects.insert(project.slug.clone(), project);
        }
        index
    }

    pub fn space(&self) -> Space {
        self.space
    }

    /// Brings the index in line with what is on disk for `change`.
    pub fn apply(&mut self, store: &Store, change: &Change) -> Update {
        let space = self.space;
        match change {
            Change::Task { project, slug } => {
                let fresh = store.task(space, project, slug);
                self.links.set(
                    ItemId::Task {
                        project: project.clone(),
                        slug: slug.clone(),
                    },
                    fresh.as_ref().map(|t| t.body.as_str()),
                );
                sync(&mut self.tasks, (project.clone(), slug.clone()), fresh)
            }
            Change::Note { project, slug } => {
                let fresh = store.note(space, project.as_deref(), slug);
                self.links.set(
                    ItemId::Note {
                        project: project.clone(),
                        slug: slug.clone(),
                    },
                    fresh.as_ref().map(|n| n.body.as_str()),
                );
                sync(&mut self.notes, (project.clone(), slug.clone()), fresh)
            }
            Change::Journal(date) => {
                let fresh = store.journal_entry(space, *date);
                self.links.set(
                    ItemId::Journal(*date),
                    fresh.as_ref().map(|e| e.body.as_str()),
                );
                sync(&mut self.journal, *date, fresh)
            }
            Change::Project(slug) => self.apply_project(store, slug),
            Change::All => {
                let fresh = SpaceIndex::scan(store, space);
                let same = self.projects == fresh.projects
                    && self.tasks == fresh.tasks
                    && self.notes == fresh.notes
                    && self.journal == fresh.journal;
                *self = fresh;
                if same {
                    Update::Unchanged
                } else {
                    Update::Upserted
                }
            }
        }
    }

    fn apply_project(&mut self, store: &Store, slug: &str) -> Update {
        let before = self.take_project_items(slug);
        let fresh = store.project(self.space, slug);
        if fresh.is_some() {
            self.load_project_items(store, slug);
        }
        let items_changed = before != self.project_items(slug);
        match sync(&mut self.projects, slug.to_string(), fresh) {
            Update::Unchanged if items_changed => Update::Upserted,
            update => update,
        }
    }

    fn load_project_items(&mut self, store: &Store, slug: &str) {
        for task in store.tasks(self.space, slug) {
            self.links.set(task_id(&task), Some(&task.body));
            self.tasks
                .insert((slug.to_string(), task.slug.clone()), task);
        }
        for note in store.notes(self.space, Some(slug)) {
            self.link_note(&note);
            self.notes
                .insert((Some(slug.to_string()), note.slug.clone()), note);
        }
    }

    fn link_note(&mut self, note: &Note) {
        self.links.set(note_id(note), Some(&note.body));
    }

    fn take_project_items(&mut self, slug: &str) -> (Vec<Task>, Vec<Note>) {
        let items = self.project_items(slug);
        for task in &items.0 {
            self.links.set(task_id(task), None);
        }
        for note in &items.1 {
            self.links.set(note_id(note), None);
        }
        self.tasks.retain(|(p, _), _| p != slug);
        self.notes.retain(|(p, _), _| p.as_deref() != Some(slug));
        items
    }

    fn project_items(&self, slug: &str) -> (Vec<Task>, Vec<Note>) {
        (
            self.tasks
                .values()
                .filter(|t| t.project == slug)
                .cloned()
                .collect(),
            self.notes
                .values()
                .filter(|n| n.project.as_deref() == Some(slug))
                .cloned()
                .collect(),
        )
    }

    /// Projects with the Inbox first, as `Store::projects` returns them.
    pub fn projects(&self) -> Vec<&Project> {
        let mut projects: Vec<&Project> = self.projects.values().collect();
        projects.sort_by_key(|p| (!p.is_inbox(), p.slug.as_str()));
        projects
    }

    pub fn project(&self, slug: &str) -> Option<&Project> {
        self.projects.get(slug)
    }

    /// The Project's Tasks in Board order, as `Store::tasks` returns them.
    pub fn tasks(&self, project: &str) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.project == project)
            .collect();
        tasks.sort_by(|a, b| crate::task_order(a, b));
        tasks
    }

    pub fn task(&self, project: &str, slug: &str) -> Option<&Task> {
        self.tasks.get(&(project.to_string(), slug.to_string()))
    }

    pub fn notes(&self, project: Option<&str>) -> Vec<&Note> {
        self.notes
            .values()
            .filter(|n| n.project.as_deref() == project)
            .collect()
    }

    pub fn note(&self, project: Option<&str>, slug: &str) -> Option<&Note> {
        self.notes
            .get(&(project.map(str::to_string), slug.to_string()))
    }

    /// Journal Entries sorted by date.
    pub fn journal(&self) -> Vec<&JournalEntry> {
        self.journal.values().collect()
    }

    pub fn journal_entry(&self, date: Date) -> Option<&JournalEntry> {
        self.journal.get(&date)
    }

    /// The Item a Link to `slug` leads to. Only Items in this Space count
    /// (ADR 0002).
    pub fn resolve(&self, slug: &str) -> LinkTarget {
        self.links.resolve(slug)
    }

    /// The Item's outgoing Links, one per distinct slug, in body order.
    pub fn links(&self, item: &ItemId) -> Vec<Link> {
        self.links.links(item)
    }

    /// The Items whose Links resolve to `item`. An Item that only shares a
    /// duplicated slug but isn't the one it resolves to has none.
    pub fn backlinks(&self, item: &ItemId) -> Vec<Backlink> {
        self.links.backlinks(item)
    }

    /// A Note's or Task's title, or a Journal Entry's date.
    pub fn title(&self, item: &ItemId) -> Option<String> {
        match item {
            ItemId::Note { project, slug } => {
                self.note(project.as_deref(), slug).map(|n| n.title.clone())
            }
            ItemId::Task { project, slug } => self.task(project, slug).map(|t| t.title.clone()),
            ItemId::Journal(date) => self.journal_entry(*date).map(|e| e.date.to_string()),
        }
    }
}

fn task_id(task: &Task) -> ItemId {
    ItemId::Task {
        project: task.project.clone(),
        slug: task.slug.clone(),
    }
}

fn note_id(note: &Note) -> ItemId {
    ItemId::Note {
        project: note.project.clone(),
        slug: note.slug.clone(),
    }
}

fn sync<K: Ord, V: PartialEq>(map: &mut BTreeMap<K, V>, key: K, fresh: Option<V>) -> Update {
    match fresh {
        Some(v) if map.get(&key) == Some(&v) => Update::Unchanged,
        Some(v) => {
            map.insert(key, v);
            Update::Upserted
        }
        None if map.remove(&key).is_some() => Update::Removed,
        None => Update::Unchanged,
    }
}

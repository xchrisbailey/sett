use std::time::SystemTime;

use serde_yaml_ng::Mapping;

use crate::callout::{self, TaskCallout};
use crate::frontmatter::Document;
use crate::{Date, Space};

/// How long a done Task stays visible in the last Column.
pub const RECENT_DONE_DAYS: i64 = 14;

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub space: Space,
    pub project: String,
    /// Fixed at creation; never derived from `title` again.
    pub slug: String,
    pub title: String,
    pub column: Option<String>,
    pub order: Option<f64>,
    pub created: Option<Date>,
    pub due: Option<Date>,
    pub done: Option<Date>,
    pub body: String,
    /// When the file was last written, by sett or anyone else. `None` for a
    /// file whose time can't be read.
    pub modified: Option<SystemTime>,
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
    /// The raw frontmatter when it couldn't be read; see [`Task::frontmatter_error`].
    pub(crate) unreadable: Option<String>,
}

impl Task {
    /// This Item with the time its file was last written.
    pub(crate) fn with_modified(mut self, modified: Option<SystemTime>) -> Task {
        self.modified = modified;
        self
    }

    /// Whether the file's frontmatter couldn't be read. sett never writes such
    /// a file, so it must be fixed in another editor.
    pub fn frontmatter_error(&self) -> bool {
        self.unreadable.is_some()
    }

    pub(crate) fn from_doc(space: Space, project: &str, slug: &str, doc: Document) -> Task {
        Task {
            space,
            project: project.to_string(),
            slug: slug.to_string(),
            title: doc.str("title").unwrap_or_else(|| slug.to_string()),
            column: doc.str("column"),
            order: doc.f64("order"),
            created: doc.date("created"),
            due: doc.date("due"),
            done: doc.date("done"),
            body: doc.body.clone(),
            modified: None,
            fields: doc.fields,
            unreadable: doc.unreadable,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
            unreadable: self.unreadable.clone(),
        };
        doc.set("title", self.title.as_str());
        doc.set_opt("column", self.column.as_deref());
        doc.set_opt("order", self.order);
        doc.set_opt("created", self.created.map(|d| d.to_string()));
        doc.set_opt("due", self.due.map(|d| d.to_string()));
        doc.set_opt("done", self.done.map(|d| d.to_string()));
        doc.body = self.body.clone();
        doc
    }

    /// `(checked, total)` over every `- [ ]` / `- [x]` line in the body,
    /// nested or not, or `None` when there are no boxes. Lines inside fenced
    /// code blocks are skipped since they are examples, not work.
    pub fn checklist(&self) -> Option<(usize, usize)> {
        let mut in_fence = false;
        let (mut checked, mut total) = (0, 0);
        for line in self.body.lines() {
            let line = line.trim_start();
            if line.starts_with("```") || line.starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            let Some(rest) = ["- ", "* ", "+ "]
                .iter()
                .find_map(|bullet| line.strip_prefix(bullet))
            else {
                continue;
            };
            if rest.starts_with("[ ]") {
                total += 1;
            } else if rest.starts_with("[x]") || rest.starts_with("[X]") {
                total += 1;
                checked += 1;
            }
        }
        (total > 0).then_some((checked, total))
    }

    /// The first Callout in the body that has text, as plain text for the
    /// Card, or `None`. Top level only (not in a list item, another quote or
    /// code) with the marker alone on its line; an empty Callout is skipped.
    pub fn first_callout(&self) -> Option<TaskCallout> {
        callout::first_callout(&self.body)
    }

    /// Whether the Task shows in the last Column without "show all": it has
    /// no `done` date (hand-edited files) or was done in the past
    /// [`RECENT_DONE_DAYS`] days.
    pub fn is_recently_done(&self, today: Date) -> bool {
        self.done
            .is_none_or(|done| today.days_since(done) <= RECENT_DONE_DAYS)
    }

    /// Moves the Task to `column`, keeping `done` in step with the rule that
    /// the last Column of the Board means done.
    pub fn move_to(&mut self, column: &str, order: f64, columns: &[String], today: Date) {
        self.column = Some(column.to_string());
        self.order = Some(order);
        self.sync_done(columns, today);
    }

    /// Sets `done` when the Task is in the last Column and clears it when it
    /// isn't, keeping an existing date. Returns whether `done` changed.
    pub(crate) fn sync_done(&mut self, columns: &[String], today: Date) -> bool {
        let in_last = columns
            .last()
            .is_some_and(|last| self.column.as_ref() == Some(last));
        let done = match self.done {
            Some(_) if in_last => self.done,
            None if in_last => Some(today),
            _ => None,
        };
        let changed = done != self.done;
        self.done = done;
        changed
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub space: Space,
    /// `None` for a Space-level Note, otherwise the owning Project's slug.
    pub project: Option<String>,
    pub slug: String,
    pub title: String,
    pub created: Option<Date>,
    pub body: String,
    /// When the file was last written, by sett or anyone else. `None` for a
    /// file whose time can't be read.
    pub modified: Option<SystemTime>,
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
    /// The raw frontmatter when it couldn't be read; see [`Note::frontmatter_error`].
    pub(crate) unreadable: Option<String>,
}

impl Note {
    /// This Item with the time its file was last written.
    pub(crate) fn with_modified(mut self, modified: Option<SystemTime>) -> Note {
        self.modified = modified;
        self
    }

    /// Whether the file's frontmatter couldn't be read. sett never writes such
    /// a file, so it must be fixed in another editor.
    pub fn frontmatter_error(&self) -> bool {
        self.unreadable.is_some()
    }

    pub(crate) fn from_doc(space: Space, project: Option<&str>, slug: &str, doc: Document) -> Note {
        Note {
            space,
            project: project.map(str::to_string),
            slug: slug.to_string(),
            title: doc.str("title").unwrap_or_else(|| slug.to_string()),
            created: doc.date("created"),
            body: doc.body.clone(),
            modified: None,
            fields: doc.fields,
            unreadable: doc.unreadable,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
            unreadable: self.unreadable.clone(),
        };
        doc.set("title", self.title.as_str());
        doc.set_opt("created", self.created.map(|d| d.to_string()));
        doc.body = self.body.clone();
        doc
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JournalEntry {
    pub space: Space,
    pub date: Date,
    pub body: String,
    /// When the file was last written, by sett or anyone else. `None` for an
    /// entry without a file, or one whose time can't be read.
    pub modified: Option<SystemTime>,
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
    /// The raw frontmatter when it couldn't be read; see [`JournalEntry::frontmatter_error`].
    pub(crate) unreadable: Option<String>,
}

impl JournalEntry {
    /// This Item with the time its file was last written.
    pub(crate) fn with_modified(mut self, modified: Option<SystemTime>) -> JournalEntry {
        self.modified = modified;
        self
    }

    /// Whether the file's frontmatter couldn't be read. sett never writes such
    /// a file, so it must be fixed in another editor.
    pub fn frontmatter_error(&self) -> bool {
        self.unreadable.is_some()
    }

    /// An empty entry held in memory only; nothing is written until it's saved.
    pub fn new(space: Space, date: Date) -> JournalEntry {
        JournalEntry {
            space,
            date,
            body: String::new(),
            modified: None,
            fields: Mapping::new(),
            unreadable: None,
        }
    }

    pub(crate) fn from_doc(space: Space, date: Date, doc: Document) -> JournalEntry {
        JournalEntry {
            space,
            date,
            body: doc.body,
            modified: None,
            fields: doc.fields,
            unreadable: doc.unreadable,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
            unreadable: self.unreadable.clone(),
        };
        doc.body = self.body.clone();
        doc
    }
}

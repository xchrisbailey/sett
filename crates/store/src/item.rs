use std::time::SystemTime;

use serde_yaml_ng::{Mapping, Value};

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
    /// Free-text Labels, already normalised when read; see [`normalize_labels`].
    pub labels: Vec<String>,
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
            labels: read_labels(&doc),
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
        let labels = normalize_labels(&self.labels);
        doc.set_opt("labels", (!labels.is_empty()).then(|| Value::from(labels)));
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

/// Whether two Labels are the same Label: equal ignoring case, so spellings
/// never split a Label in two.
pub fn same_label(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Trims each Label, drops empty ones and later duplicates that match ignoring
/// case. The first spelling wins and the order is kept, so hand-edited files
/// and sett agree on what a Task's Labels are.
pub fn normalize_labels<S: AsRef<str>>(labels: &[S]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for label in labels.iter().map(|l| l.as_ref().trim()) {
        if !label.is_empty() && !kept.iter().any(|k| same_label(k, label)) {
            kept.push(label.to_string());
        }
    }
    kept
}

/// Every Label used by `tasks` (done ones included), once per case-insensitive
/// spelling, sorted ignoring case. Serves a Project's filter options and a
/// Space's suggestions alike.
pub fn labels_of<'a>(tasks: impl IntoIterator<Item = &'a Task>) -> Vec<String> {
    let all: Vec<&str> = tasks
        .into_iter()
        .flat_map(|t| t.labels.iter().map(String::as_str))
        .collect();
    let mut labels = normalize_labels(&all);
    labels.sort_by_key(|l| l.to_lowercase());
    labels
}

/// A YAML list of strings, or a bare string as one Label. Hand-edited files
/// hold all sorts, so anything else is ignored rather than rejected.
fn read_labels(doc: &Document) -> Vec<String> {
    match doc.fields.get("labels") {
        Some(Value::String(s)) => normalize_labels(&[s]),
        Some(Value::Sequence(seq)) => {
            let strings: Vec<&str> = seq.iter().filter_map(Value::as_str).collect();
            normalize_labels(&strings)
        }
        _ => Vec::new(),
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

/// A body to start a new Item from, kept in `<space>/templates/`. Not an Item:
/// it has no Links and appears in no list of Items.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub space: Space,
    pub slug: String,
    pub title: String,
    /// Holds no frontmatter.
    pub body: String,
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order. Never copied into a new Item.
    pub(crate) fields: Mapping,
    /// The raw frontmatter when it couldn't be read; see [`Template::frontmatter_error`].
    pub(crate) unreadable: Option<String>,
}

impl Template {
    /// Whether the file's frontmatter couldn't be read. sett never writes such
    /// a file, so it must be fixed in another editor.
    pub fn frontmatter_error(&self) -> bool {
        self.unreadable.is_some()
    }

    pub(crate) fn from_doc(space: Space, slug: &str, doc: Document) -> Template {
        Template {
            space,
            slug: slug.to_string(),
            title: doc.str("title").unwrap_or_else(|| slug.to_string()),
            body: doc.body,
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
        doc.body = self.body.clone();
        doc
    }
}

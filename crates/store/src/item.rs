use serde_yaml_ng::Mapping;

use crate::frontmatter::Document;
use crate::{Date, Space};

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
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
}

impl Task {
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
            fields: doc.fields,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
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

    /// Moves the Task to `column`, keeping `done` in step with the rule that
    /// the last Column of the Board means done.
    pub fn move_to(&mut self, column: &str, order: f64, columns: &[String], today: Date) {
        let entering_last = columns.last().is_some_and(|last| last == column);
        if entering_last {
            if self.done.is_none() {
                self.done = Some(today);
            }
        } else {
            self.done = None;
        }
        self.column = Some(column.to_string());
        self.order = Some(order);
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
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
}

impl Note {
    pub(crate) fn from_doc(space: Space, project: Option<&str>, slug: &str, doc: Document) -> Note {
        Note {
            space,
            project: project.map(str::to_string),
            slug: slug.to_string(),
            title: doc.str("title").unwrap_or_else(|| slug.to_string()),
            created: doc.date("created"),
            body: doc.body.clone(),
            fields: doc.fields,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
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
    /// Every frontmatter key as last read or written, so unknown keys survive
    /// a save in their original order.
    pub(crate) fields: Mapping,
}

impl JournalEntry {
    /// An empty entry held in memory only; nothing is written until it's saved.
    pub fn new(space: Space, date: Date) -> JournalEntry {
        JournalEntry {
            space,
            date,
            body: String::new(),
            fields: Mapping::new(),
        }
    }

    pub(crate) fn from_doc(space: Space, date: Date, doc: Document) -> JournalEntry {
        JournalEntry {
            space,
            date,
            body: doc.body,
            fields: doc.fields,
        }
    }

    pub(crate) fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: String::new(),
        };
        doc.body = self.body.clone();
        doc
    }
}

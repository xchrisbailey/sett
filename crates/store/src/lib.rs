//! On-disk data layer for sett (ADR 0001): Items are Markdown files with YAML
//! frontmatter under `<root>/<space>/`.
//!
//! Loading never fails and never modifies files. Writing only happens through
//! the explicit `create_*`, `save_*` and `trash_*` calls, so callers control
//! that an Item is written only after it was edited (ADR 0003).

mod board;
mod date;
mod frontmatter;
mod index;
mod item;
mod links;
mod settings;
mod slug;
mod watch;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_yaml_ng::{Mapping, Value};

pub use board::{Board, BoardColumn, MIN_COLUMNS};
pub use date::Date;
use frontmatter::Document;
pub use index::{Change, SpaceIndex, Update};
pub use item::{JournalEntry, Note, RECENT_DONE_DAYS, Task};
pub use links::{InboundLink, ItemId, Link, LinkTarget};
pub use settings::{EditorMode, Settings};
pub use watch::Watcher;
use watch::WriteLog;

pub const INBOX_SLUG: &str = "inbox";
const DEFAULT_ICON: &str = "folder";
const DEFAULT_COLUMNS: [&str; 3] = ["Todo", "Doing", "Done"];
const AGENTS_MD: &str = include_str!("agents.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Space {
    Personal,
    Work,
}

impl Space {
    pub const ALL: [Space; 2] = [Space::Personal, Space::Work];

    pub fn dir_name(self) -> &'static str {
        match self {
            Space::Personal => "personal",
            Space::Work => "work",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub space: Space,
    pub slug: String,
    pub name: String,
    /// A Lucide icon name.
    pub icon: Option<String>,
    pub columns: Vec<String>,
    pub archived: bool,
    pub body: String,
    fields: Mapping,
    /// The raw frontmatter when it couldn't be read; see [`Project::frontmatter_error`].
    unreadable: Option<String>,
}

impl Project {
    pub fn is_inbox(&self) -> bool {
        self.slug == INBOX_SLUG
    }

    /// Whether `project.md`'s frontmatter couldn't be read. sett never writes
    /// such a Project, so its name, icon and Columns can't be changed in sett.
    pub fn frontmatter_error(&self) -> bool {
        self.unreadable.is_some()
    }

    fn from_doc(space: Space, slug: &str, doc: Document) -> Project {
        Project {
            space,
            slug: slug.to_string(),
            name: doc.str("name").unwrap_or_else(|| slug.to_string()),
            icon: doc.str("icon"),
            columns: doc
                .strings("columns")
                .unwrap_or_else(|| DEFAULT_COLUMNS.map(String::from).to_vec()),
            archived: doc.bool("archived").unwrap_or(false),
            body: doc.body,
            fields: doc.fields,
            unreadable: doc.unreadable,
        }
    }

    fn to_doc(&self) -> Document {
        let mut doc = Document {
            fields: self.fields.clone(),
            body: self.body.clone(),
            unreadable: self.unreadable.clone(),
        };
        doc.set("name", self.name.as_str());
        doc.set_opt("icon", self.icon.as_deref());
        doc.set(
            "columns",
            self.columns
                .iter()
                .map(|c| Value::from(c.as_str()))
                .collect::<Vec<_>>(),
        );
        doc.set("archived", self.archived);
        doc
    }
}

/// Moves a file or folder out of the data root.
type TrashFn = Arc<dyn Fn(&Path) -> io::Result<()> + Send + Sync>;

pub struct Store {
    root: PathBuf,
    log: WriteLog,
    trash: TrashFn,
}

impl Store {
    /// Opens the data root, running first-run setup if it doesn't exist yet.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Store> {
        let store = Store {
            root: root.into(),
            log: WriteLog::default(),
            // Tests must never reach the real Trash, so it's opt-in: the app
            // calls `use_system_trash`, and tests that trash call `set_trash`.
            trash: Arc::new(|path| {
                Err(io::Error::other(format!(
                    "no Trash set for {}: call Store::use_system_trash or Store::set_trash",
                    path.display()
                )))
            }),
        };
        if !store.root.exists() {
            store.first_run()?;
        }
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Makes `trash_*` move files to the macOS Trash. Until this or
    /// [`Store::set_trash`] is called, trashing fails.
    pub fn use_system_trash(&mut self) {
        self.set_trash(|path| trash::delete(path).map_err(io::Error::other));
    }

    /// Replaces how `trash_*` disposes of files. Tests use this to keep the
    /// real Trash untouched.
    pub fn set_trash(&mut self, trash: impl Fn(&Path) -> io::Result<()> + Send + Sync + 'static) {
        self.trash = Arc::new(trash);
    }

    fn first_run(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        for space in Space::ALL {
            self.create_dirs(&self.notes_dir(space, None))?;
            self.create_dirs(&self.journal_dir(space))?;
            let mut inbox = Project::from_doc(space, INBOX_SLUG, Document::default());
            inbox.name = "Inbox".to_string();
            inbox.icon = Some("inbox".to_string());
            self.write_project(&inbox)?;
        }
        let agents = self.root.join("AGENTS.md");
        if !agents.exists() {
            fs::write(agents, AGENTS_MD)?;
        }
        Ok(())
    }

    /// Every Project folder in the Space, sorted with the Inbox first. Folders
    /// with a missing or broken `project.md` still load, using defaults.
    pub fn projects(&self, space: Space) -> Vec<Project> {
        let mut projects: Vec<Project> = subdirs(&self.projects_dir(space))
            .into_iter()
            .map(|(slug, dir)| {
                let doc = read_doc(&dir.join("project.md")).unwrap_or_default();
                Project::from_doc(space, &slug, doc)
            })
            .collect();
        projects.sort_by(|a, b| (!a.is_inbox(), &a.slug).cmp(&(!b.is_inbox(), &b.slug)));
        projects
    }

    /// The Project's Tasks sorted by `order`, with unordered Tasks last.
    pub fn tasks(&self, space: Space, project: &str) -> Vec<Task> {
        let dir = self.project_dir(space, project).join("tasks");
        let mut tasks: Vec<Task> = markdown_files(&dir)
            .into_iter()
            .map(|(slug, doc)| {
                Task::from_doc(space, project, &slug, doc)
                    .with_modified(modified(&dir.join(md(&slug))))
            })
            .collect();
        tasks.sort_by(task_order);
        tasks
    }

    /// The Project if its folder exists, using defaults for a missing or
    /// broken `project.md`.
    pub fn project(&self, space: Space, slug: &str) -> Option<Project> {
        let dir = self.project_dir(space, slug);
        if !slug::is_safe(slug) || !dir.is_dir() {
            return None;
        }
        let doc = read_doc(&dir.join("project.md")).unwrap_or_default();
        Some(Project::from_doc(space, slug, doc))
    }

    pub fn task(&self, space: Space, project: &str, slug: &str) -> Option<Task> {
        if !slug::is_safe(project) || !slug::is_safe(slug) {
            return None;
        }
        let path = self
            .project_dir(space, project)
            .join("tasks")
            .join(md(slug));
        Some(Task::from_doc(space, project, slug, read_doc(&path)?).with_modified(modified(&path)))
    }

    pub fn note(&self, space: Space, project: Option<&str>, slug: &str) -> Option<Note> {
        if !project.is_none_or(slug::is_safe) || !slug::is_safe(slug) {
            return None;
        }
        let path = self.notes_dir(space, project).join(md(slug));
        Some(Note::from_doc(space, project, slug, read_doc(&path)?).with_modified(modified(&path)))
    }

    pub fn journal_entry(&self, space: Space, date: Date) -> Option<JournalEntry> {
        let path = self.journal_dir(space).join(md(&date.to_string()));
        Some(JournalEntry::from_doc(space, date, read_doc(&path)?).with_modified(modified(&path)))
    }

    /// Space-level Notes when `project` is `None`, otherwise that Project's.
    pub fn notes(&self, space: Space, project: Option<&str>) -> Vec<Note> {
        let dir = self.notes_dir(space, project);
        markdown_files(&dir)
            .into_iter()
            .map(|(slug, doc)| {
                Note::from_doc(space, project, &slug, doc)
                    .with_modified(modified(&dir.join(md(&slug))))
            })
            .collect()
    }

    /// Journal Entries sorted by date. Files whose name isn't a valid
    /// `YYYY-MM-DD` date are skipped.
    pub fn journal(&self, space: Space) -> Vec<JournalEntry> {
        let dir = self.journal_dir(space);
        let mut entries: Vec<JournalEntry> = markdown_files(&dir)
            .into_iter()
            .filter_map(|(stem, doc)| {
                Some(
                    JournalEntry::from_doc(space, Date::parse(&stem)?, doc)
                        .with_modified(modified(&dir.join(md(&stem)))),
                )
            })
            .collect();
        entries.sort_by_key(|e| e.date);
        entries
    }

    /// Creates a Project with the default Columns. Its slug comes from `name`,
    /// and it gets the folder icon unless `icon` names another.
    pub fn create_project(
        &self,
        space: Space,
        name: &str,
        icon: Option<&str>,
    ) -> io::Result<Project> {
        let dir = self.projects_dir(space);
        let slug = slug::unique_slug(&slug::slugify(name), |s| dir.join(s).exists());
        let mut project = Project::from_doc(space, &slug, Document::default());
        project.name = name.to_string();
        project.icon = Some(icon.unwrap_or(DEFAULT_ICON).to_string());
        self.write_project(&project)?;
        project.fields = project.to_doc().fields;
        Ok(project)
    }

    /// Writes a Task at the end of the Project's first Column.
    pub fn create_task(
        &self,
        space: Space,
        project: &str,
        title: &str,
        today: Date,
    ) -> io::Result<Task> {
        let columns = self.columns_of(space, project)?;
        let first = columns.first().cloned();
        let last_order = self
            .tasks(space, project)
            .iter()
            .filter(|t| t.column == first)
            .filter_map(|t| t.order)
            .fold(0.0, f64::max);
        let mut task = self.blank_task(space, project, title, today);
        task.column = first;
        task.order = Some(last_order + 1.0);
        self.write_task_new(task)
    }

    /// Writes a Task at the top of Column `column`, where the quick-add box
    /// that creates it sits. In the last Column it is created done.
    pub fn create_task_in(
        &self,
        space: Space,
        project: &str,
        column: &str,
        title: &str,
        today: Date,
    ) -> io::Result<Task> {
        let columns = self.columns_of(space, project)?;
        if !columns.iter().any(|c| c == column) {
            return Err(not_found(format!("column {column}")));
        }
        let tasks = self.tasks(space, project);
        let first_order = Board::new(&columns, &tasks)
            .column(column)
            .and_then(|c| c.tasks().iter().find_map(|t| t.order))
            .unwrap_or(1.0);
        let mut task = self.blank_task(space, project, title, today);
        task.move_to(column, first_order - 1.0, &columns, today);
        self.write_task_new(task)
    }

    /// Refuses a Project whose frontmatter can't be read, since its real
    /// Columns aren't known and a new Task would land in a default one.
    fn columns_of(&self, space: Space, project: &str) -> io::Result<Vec<String>> {
        check_slug(project)?;
        let project = self
            .project(space, project)
            .ok_or_else(|| not_found(format!("project {project}")))?;
        if project.frontmatter_error() {
            return Err(unreadable_frontmatter(
                &self.project_dir(space, &project.slug).join("project.md"),
            ));
        }
        Ok(project.columns)
    }

    /// A Task with a fresh slug from `title`, not yet written.
    fn blank_task(&self, space: Space, project: &str, title: &str, today: Date) -> Task {
        let slug = self.unique_item_slug(space, title);
        let mut task = Task::from_doc(space, project, &slug, Document::default());
        task.title = title.to_string();
        task.created = Some(today);
        task
    }

    fn write_task_new(&self, mut task: Task) -> io::Result<Task> {
        let dir = self.project_dir(task.space, &task.project).join("tasks");
        self.write_new(&dir, &task.slug, task.to_doc())?;
        task.fields = task.to_doc().fields;
        task.modified = modified(&dir.join(md(&task.slug)));
        Ok(task)
    }

    /// Moves the Task `slug` into Column `to`, before the Task `before` or at
    /// the end, and returns the Tasks it wrote. Only the moved Task's file is
    /// written unless its new neighbours have no room left between their
    /// orders, in which case the Column is renumbered.
    pub fn move_task(
        &self,
        board: &Board,
        slug: &str,
        to: &str,
        before: Option<&str>,
        today: Date,
    ) -> io::Result<Vec<Task>> {
        let plan = board
            .plan_move(slug, to, before)
            .ok_or_else(|| not_found(format!("task {slug} or column {to}")))?;
        let mut written = Vec::with_capacity(plan.len());
        for (task, order) in plan {
            let mut task = task.clone();
            if task.slug == slug {
                task.move_to(to, order, board.column_names(), today);
            } else {
                task.column = Some(to.to_string());
                task.order = Some(order);
            }
            self.save_task(&mut task)?;
            written.push(task);
        }
        Ok(written)
    }

    pub fn create_note(
        &self,
        space: Space,
        project: Option<&str>,
        title: &str,
        today: Date,
    ) -> io::Result<Note> {
        if let Some(p) = project {
            check_slug(p)?;
        }
        let dir = self.notes_dir(space, project);
        let slug = self.unique_item_slug(space, title);
        let mut note = Note::from_doc(space, project, &slug, Document::default());
        note.title = title.to_string();
        note.created = Some(today);
        self.write_new(&dir, &slug, note.to_doc())?;
        note.fields = note.to_doc().fields;
        note.modified = modified(&dir.join(md(&slug)));
        Ok(note)
    }

    /// A slug from `title` that no Note or Task anywhere in the Space has,
    /// archived Projects included, so a Link to it is never ambiguous.
    /// Date-shaped slugs are left to Journal Entries.
    fn unique_item_slug(&self, space: Space, title: &str) -> String {
        let mut dirs = vec![self.notes_dir(space, None)];
        for (_, project) in subdirs(&self.projects_dir(space)) {
            dirs.push(project.join("notes"));
            dirs.push(project.join("tasks"));
        }
        let taken: HashSet<String> = dirs.iter().flat_map(|d| markdown_stems(d)).collect();
        slug::unique_slug(&slug::slugify(title), |s| {
            taken.contains(s) || Date::parse(s).is_some()
        })
    }

    pub fn save_project(&self, project: &mut Project) -> io::Result<()> {
        if project.is_inbox() && project.archived {
            return Err(invalid("the Inbox can't be archived"));
        }
        self.write_project(project)?;
        project.fields = project.to_doc().fields;
        Ok(())
    }

    pub fn save_task(&self, task: &mut Task) -> io::Result<()> {
        check_slug(&task.project)?;
        check_slug(&task.slug)?;
        let dir = self.project_dir(task.space, &task.project).join("tasks");
        let doc = task.to_doc();
        let path = dir.join(md(&task.slug));
        self.write_doc(&path, &doc)?;
        task.fields = doc.fields;
        task.modified = modified(&path);
        Ok(())
    }

    pub fn save_note(&self, note: &mut Note) -> io::Result<()> {
        if let Some(p) = &note.project {
            check_slug(p)?;
        }
        check_slug(&note.slug)?;
        let dir = self.notes_dir(note.space, note.project.as_deref());
        let doc = note.to_doc();
        let path = dir.join(md(&note.slug));
        self.write_doc(&path, &doc)?;
        note.fields = doc.fields;
        note.modified = modified(&path);
        Ok(())
    }

    pub fn save_journal_entry(&self, entry: &mut JournalEntry) -> io::Result<()> {
        let doc = entry.to_doc();
        let path = self.journal_path(entry);
        self.write_doc(&path, &doc)?;
        entry.fields = doc.fields;
        entry.modified = modified(&path);
        Ok(())
    }

    /// Moves the Project folder, Tasks and Notes included, to the macOS Trash.
    pub fn trash_project(&self, project: &Project) -> io::Result<()> {
        if project.is_inbox() {
            return Err(invalid("the Inbox can't be deleted"));
        }
        check_slug(&project.slug)?;
        self.trash(&self.project_dir(project.space, &project.slug))
    }

    pub fn trash_task(&self, task: &Task) -> io::Result<()> {
        check_slug(&task.project)?;
        check_slug(&task.slug)?;
        let dir = self.project_dir(task.space, &task.project).join("tasks");
        self.trash(&dir.join(md(&task.slug)))
    }

    pub fn trash_note(&self, note: &Note) -> io::Result<()> {
        if let Some(p) = &note.project {
            check_slug(p)?;
        }
        check_slug(&note.slug)?;
        self.trash(
            &self
                .notes_dir(note.space, note.project.as_deref())
                .join(md(&note.slug)),
        )
    }

    pub fn trash_journal_entry(&self, entry: &JournalEntry) -> io::Result<()> {
        self.trash(&self.journal_path(entry))
    }

    fn write_project(&self, project: &Project) -> io::Result<()> {
        check_slug(&project.slug)?;
        let dir = self.project_dir(project.space, &project.slug);
        self.create_dirs(&dir.join("tasks"))?;
        self.create_dirs(&dir.join("notes"))?;
        self.write_doc(&dir.join("project.md"), &project.to_doc())
    }

    /// Uses `create_new` so a file that appeared since the slug was picked
    /// (an agent writing at the same moment) is never overwritten.
    fn write_new(&self, dir: &Path, slug: &str, doc: Document) -> io::Result<()> {
        use std::io::Write;
        self.create_dirs(dir)?;
        let path = dir.join(md(slug));
        let text = doc.render();
        self.log.file(self.rel(&path), text.as_bytes());
        fs::File::create_new(path)?.write_all(text.as_bytes())
    }

    /// Writes via a hidden temp file and a rename, so neither the watcher nor
    /// an agent ever reads a half-written file.
    ///
    /// Refuses a document whose frontmatter couldn't be read: rewriting it
    /// would replace the user's YAML with defaults (ADR 0001).
    fn write_doc(&self, path: &Path, doc: &Document) -> io::Result<()> {
        if doc.unreadable.is_some() {
            return Err(unreadable_frontmatter(path));
        }
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(invalid(format!("invalid path {path:?}")));
        };
        self.create_dirs(dir)?;
        let tmp = dir.join(format!(".{}.sett-tmp", name.to_string_lossy()));
        let text = doc.render();
        fs::write(&tmp, &text)?;
        // Logged before the rename so the watcher can't see the new content
        // before it knows sett wrote it.
        self.log.file(self.rel(path), text.as_bytes());
        fs::rename(&tmp, path)
    }

    fn create_dirs(&self, dir: &Path) -> io::Result<()> {
        for missing in dir.ancestors().take_while(|d| !d.exists()) {
            self.log.dir(self.rel(missing));
        }
        fs::create_dir_all(dir)
    }

    fn trash(&self, path: &Path) -> io::Result<()> {
        self.log.removed(self.rel(path), path);
        (self.trash)(path)
    }

    /// The write log is keyed relative to the root so it matches the
    /// watcher's resolved paths whatever form `root` was given in.
    fn rel(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.root).unwrap_or(path).to_path_buf()
    }

    /// Starts watching one Space for changes made outside sett. sett's own
    /// writes through this `Store` are not reported.
    pub fn watch(&self, space: Space) -> io::Result<Watcher> {
        Watcher::start(&self.root, space, self.log.clone())
    }

    fn space_dir(&self, space: Space) -> PathBuf {
        self.root.join(space.dir_name())
    }

    fn projects_dir(&self, space: Space) -> PathBuf {
        self.space_dir(space).join("projects")
    }

    fn project_dir(&self, space: Space, slug: &str) -> PathBuf {
        self.projects_dir(space).join(slug)
    }

    fn notes_dir(&self, space: Space, project: Option<&str>) -> PathBuf {
        match project {
            Some(p) => self.project_dir(space, p).join("notes"),
            None => self.space_dir(space).join("notes"),
        }
    }

    fn journal_dir(&self, space: Space) -> PathBuf {
        self.space_dir(space).join("journal")
    }

    fn journal_path(&self, entry: &JournalEntry) -> PathBuf {
        self.journal_dir(entry.space)
            .join(md(&entry.date.to_string()))
    }
}

/// Board order: by `order`, with unordered Tasks last.
fn task_order(a: &Task, b: &Task) -> std::cmp::Ordering {
    let key = |t: &Task| t.order.unwrap_or(f64::INFINITY);
    key(a).total_cmp(&key(b)).then_with(|| a.slug.cmp(&b.slug))
}

fn modified(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn md(slug: &str) -> String {
    format!("{slug}.md")
}

/// The stem of every `*.md` file in `dir`, without reading them.
fn markdown_stems(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            Some(name.strip_suffix(".md")?.to_string())
        })
        .collect()
}

fn check_slug(slug: &str) -> io::Result<()> {
    if slug::is_safe(slug) {
        Ok(())
    } else {
        Err(invalid(format!("invalid slug {slug:?}")))
    }
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn unreadable_frontmatter(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "the frontmatter of {} can't be read, so sett won't save it",
            path.display()
        ),
    )
}

fn not_found(what: String) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{what} not found"))
}

/// Unreadable or non-UTF-8 files are treated as absent rather than as errors,
/// so one bad file can't stop a Space from loading.
fn read_doc(path: &Path) -> Option<Document> {
    fs::read_to_string(path).ok().map(|t| Document::parse(&t))
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| Some((e.file_name().into_string().ok()?, e.path())))
        .filter(|(name, _)| !name.starts_with('.'))
        .collect()
}

/// Every readable `*.md` file in `dir` as `(stem, document)`, sorted by stem.
fn markdown_files(dir: &Path) -> Vec<(String, Document)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, Document)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".md")?;
            if stem.is_empty() || stem.starts_with('.') {
                return None;
            }
            Some((stem.to_string(), read_doc(&e.path())?))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

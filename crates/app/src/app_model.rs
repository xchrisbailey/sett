use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Task};
use sett_store::{
    Board, Change, Date, EditorMode, INBOX_SLUG, ItemId, JournalEntry, Note, Project, Settings,
    Space, SpaceIndex, Store, Update, Watcher,
};

/// What the center panel shows for the active Space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Journal,
    Notes,
    /// A Project, by slug.
    Project(String),
}

impl Default for Selection {
    fn default() -> Self {
        Selection::Project(INBOX_SLUG.to_string())
    }
}

/// What the AppModel tells the views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppModelEvent {
    /// Show this Item of the active Space. The selection already points at
    /// where it lives.
    OpenItem(ItemId),
}

/// The open data root and the active Space: its index, kept current by a
/// watcher, and each Space's last selection. Only the active Space is indexed
/// and watched, so nothing from the other Space is held (ADR 0002).
pub struct AppModel {
    store: Store,
    settings: Settings,
    settings_path: Option<PathBuf>,
    index: SpaceIndex,
    selections: HashMap<Space, Selection>,
    /// `None` when the data root isn't watched, or watching it failed.
    watching: Option<Watching>,
}

impl EventEmitter<AppModelEvent> for AppModel {}

struct Watching {
    _watcher: Watcher,
    _task: Task<()>,
}

impl AppModel {
    /// Opens the data root from `settings` (running first-run setup if
    /// needed) in the last used Space. `settings_path` is where the last
    /// Space is saved; `None` skips saving.
    pub fn open(
        settings: Settings,
        settings_path: Option<PathBuf>,
        cx: &mut App,
    ) -> io::Result<Entity<AppModel>> {
        AppModel::open_with(settings, settings_path, true, cx)
    }

    /// Like [`AppModel::open`] but without watching the data root, for when
    /// the caller reports outside changes itself through
    /// [`AppModel::apply_change`]. GPUI's test executor rejects wakeups from
    /// the watcher's thread, so UI tests use this.
    pub fn open_unwatched(
        settings: Settings,
        settings_path: Option<PathBuf>,
        cx: &mut App,
    ) -> io::Result<Entity<AppModel>> {
        AppModel::open_with(settings, settings_path, false, cx)
    }

    fn open_with(
        settings: Settings,
        settings_path: Option<PathBuf>,
        watched: bool,
        cx: &mut App,
    ) -> io::Result<Entity<AppModel>> {
        let store = Store::open(&settings.data_root)?;
        Ok(cx.new(|cx| {
            let space = settings.last_space;
            let watching = watched.then(|| watch(&store, space, cx)).flatten();
            let index = SpaceIndex::scan(&store, space);
            AppModel {
                store,
                settings,
                settings_path,
                index,
                selections: HashMap::new(),
                watching,
            }
        }))
    }

    pub fn space(&self) -> Space {
        self.index.space()
    }

    pub fn index(&self) -> &SpaceIndex {
        &self.index
    }

    pub fn selection(&self) -> Selection {
        self.selections
            .get(&self.space())
            .cloned()
            .unwrap_or_default()
    }

    pub fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if self.selection() != selection {
            self.selections.insert(self.space(), selection);
            cx.notify();
        }
    }

    /// Selects where an Item of the active Space lives and asks the view
    /// there to open it, as following a Link does.
    pub fn open_item(&mut self, item: ItemId, cx: &mut Context<Self>) {
        let selection = match &item {
            ItemId::Note { project: None, .. } => Selection::Notes,
            ItemId::Note {
                project: Some(project),
                ..
            }
            | ItemId::Task { project, .. } => Selection::Project(project.clone()),
            ItemId::Journal(_) => Selection::Journal,
        };
        self.selections.insert(self.space(), selection);
        cx.emit(AppModelEvent::OpenItem(item));
        cx.notify();
    }

    pub fn switch_space(&mut self, space: Space, cx: &mut Context<Self>) {
        if space == self.space() {
            return;
        }
        // Drop the old watcher first so no late event from it reaches the new index.
        if self.watching.take().is_some() {
            self.watching = watch(&self.store, space, cx);
        }
        self.index = SpaceIndex::scan(&self.store, space);
        self.fix_selection();
        self.save_last_space(space);
        cx.notify();
    }

    /// Creates a Project in the active Space and opens it. Returns its slug.
    pub fn create_project(
        &mut self,
        name: &str,
        icon: Option<&str>,
        cx: &mut Context<Self>,
    ) -> io::Result<String> {
        let project = self.store.create_project(self.space(), name, icon)?;
        let slug = project.slug;
        self.reload_project(&slug, cx);
        self.select(Selection::Project(slug.clone()), cx);
        Ok(slug)
    }

    /// Renames a Project or changes its icon. The slug, and so its folder,
    /// stays the same.
    pub fn update_project(
        &mut self,
        slug: &str,
        name: &str,
        icon: Option<&str>,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_project(slug, cx, |project| {
            project.name = name.to_string();
            project.icon = icon.map(str::to_string);
        })
    }

    /// Archiving only flags the Project in `project.md`; its files stay put.
    pub fn set_archived(
        &mut self,
        slug: &str,
        archived: bool,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_project(slug, cx, |project| project.archived = archived)
    }

    /// Moves the Project's folder to the Trash. Links into it are left as
    /// they are.
    pub fn trash_project(&mut self, slug: &str, cx: &mut Context<Self>) -> io::Result<()> {
        let project = self.find_project(slug)?.clone();
        self.store.trash_project(&project)?;
        self.reload_project(slug, cx);
        Ok(())
    }

    /// Replaces how deleted Projects are disposed of, which by default moves
    /// them to the macOS Trash. Tests use this to keep the real Trash untouched.
    pub fn set_trash(&mut self, trash: impl Fn(&Path) -> io::Result<()> + Send + Sync + 'static) {
        self.store.set_trash(trash);
    }

    fn edit_project(
        &mut self,
        slug: &str,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Project),
    ) -> io::Result<()> {
        let mut project = self.find_project(slug)?.clone();
        edit(&mut project);
        self.store.save_project(&mut project)?;
        self.reload_project(slug, cx);
        Ok(())
    }

    fn find_project(&self, slug: &str) -> io::Result<&Project> {
        self.index.project(slug).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("project {slug} not found"))
        })
    }

    /// sett's own writes aren't reported by the watcher, so the index is
    /// brought up to date here instead.
    fn reload_project(&mut self, slug: &str, cx: &mut Context<Self>) {
        self.apply_change(Change::Project(slug.to_string()), cx);
    }

    pub fn editor_mode(&self) -> EditorMode {
        self.settings.editor_mode
    }

    /// Switches every editor between the block view and raw Markdown, and
    /// remembers the choice in `settings.toml`.
    pub fn toggle_editor_mode(&mut self, cx: &mut Context<Self>) {
        self.settings.editor_mode = match self.settings.editor_mode {
            EditorMode::Block => EditorMode::Raw,
            EditorMode::Raw => EditorMode::Block,
        };
        self.save_settings();
        cx.notify();
    }

    /// Creates a Note in the active Space, in `project` or at Space level.
    /// Returns its slug, which stays fixed when the Note is renamed.
    pub fn create_note(
        &mut self,
        project: Option<&str>,
        title: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<String> {
        let note = self
            .store
            .create_note(self.space(), project, title, today())?;
        self.apply_change(note_change(project, &note.slug), cx);
        Ok(note.slug)
    }

    /// Writes a Note. It may belong to a Space that is no longer active when
    /// an edit is saved on the way out of it.
    pub fn save_note(&mut self, note: &mut Note, cx: &mut Context<Self>) -> io::Result<()> {
        self.store.save_note(note)?;
        if note.space == self.space() {
            self.apply_change(note_change(note.project.as_deref(), &note.slug), cx);
        }
        Ok(())
    }

    /// Writes a Journal Entry, creating its file the first time.
    pub fn save_journal_entry(
        &mut self,
        entry: &mut JournalEntry,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.store.save_journal_entry(entry)?;
        if entry.space == self.space() {
            self.apply_change(Change::Journal(entry.date), cx);
        }
        Ok(())
    }

    /// Moves a Note of the active Space to the Trash.
    pub fn trash_note(
        &mut self,
        project: Option<&str>,
        slug: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        let note = self.index.note(project, slug).cloned().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("note {slug} not found"))
        })?;
        self.store.trash_note(&note)?;
        self.apply_change(note_change(project, slug), cx);
        Ok(())
    }

    /// Brings the active Space's index in line with an outside change to it.
    pub fn apply_change(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.index.apply(&self.store, &change) != Update::Unchanged {
            self.fix_selection();
            cx.notify();
        }
    }

    /// Quick-adds a Task at the top of a Column of the Project's Board.
    pub fn create_task(
        &mut self,
        project: &str,
        column: &str,
        title: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        let task = self
            .store
            .create_task_in(self.space(), project, column, title, today())?;
        self.reload(vec![task_change(project, &task.slug)], cx);
        Ok(())
    }

    /// Moves a Task on its Project's Board into `column`, before the Task
    /// `before` or at the end. This is what dropping a Card does.
    pub fn move_task(
        &mut self,
        project: &str,
        slug: &str,
        column: &str,
        before: Option<&str>,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        let columns = self.find_project(project)?.columns.clone();
        let board = Board::new(&columns, self.index.tasks(project));
        let written = self
            .store
            .move_task(&board, slug, column, before, today())?;
        let changes: Vec<Change> = written
            .iter()
            .map(|t| task_change(project, &t.slug))
            .collect();
        self.reload(changes, cx);
        Ok(())
    }

    /// Writes a Task. It may belong to a Space that is no longer active when
    /// an edit is saved on the way out of it.
    pub fn save_task(
        &mut self,
        task: &mut sett_store::Task,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.store.save_task(task)?;
        if task.space == self.space() {
            self.apply_change(task_change(&task.project, &task.slug), cx);
        }
        Ok(())
    }

    /// Sets or clears a Task's due date, writing only that Task's file.
    pub fn set_task_due(
        &mut self,
        project: &str,
        slug: &str,
        due: Option<Date>,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        let mut task = self.index.task(project, slug).cloned().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("task {slug} not found"))
        })?;
        if task.due == due {
            return Ok(());
        }
        task.due = due;
        self.save_task(&mut task, cx)
    }

    pub fn add_column(
        &mut self,
        project: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_columns(project, cx, |store, project, _| {
            store.add_column(project, name)
        })
    }

    pub fn rename_column(
        &mut self,
        project: &str,
        from: &str,
        to: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_columns(project, cx, |store, project, tasks| {
            store.rename_column(project, tasks, from, to)
        })
    }

    /// Moves the Column at `from` to position `to` (both zero-based).
    pub fn move_column(
        &mut self,
        project: &str,
        from: usize,
        to: usize,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_columns(project, cx, |store, project, tasks| {
            store.move_column(project, tasks, from, to, today())
        })
    }

    pub fn delete_column(
        &mut self,
        project: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        self.edit_columns(project, cx, |store, project, tasks| {
            store.delete_column(project, tasks, name, today())
        })
    }

    /// Runs a Column edit, then reloads the whole Project, since an edit can
    /// rewrite any of its Tasks.
    fn edit_columns(
        &mut self,
        slug: &str,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&Store, &mut Project, Vec<&sett_store::Task>) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut project = self.find_project(slug)?.clone();
        let result = edit(&self.store, &mut project, self.index.tasks(slug));
        // Reload even on failure: an edit can fail after writing some files.
        self.reload_project(slug, cx);
        result
    }

    /// Like [`AppModel::reload_project`], for just the Tasks written.
    fn reload(&mut self, changes: Vec<Change>, cx: &mut Context<Self>) {
        for change in changes {
            self.apply_change(change, cx);
        }
    }

    /// A selected Project that was deleted or renamed on disk falls back to
    /// the Inbox rather than leaving the center panel pointing at nothing.
    fn fix_selection(&mut self) {
        if let Selection::Project(slug) = self.selection()
            && self.index.project(&slug).is_none()
        {
            self.selections.remove(&self.space());
        }
    }

    fn save_last_space(&mut self, space: Space) {
        self.settings.last_space = space;
        self.save_settings();
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path
            && let Err(err) = self.settings.save(path)
        {
            // Not worth interrupting the user: the worst case is reopening in
            // the other Space or editor mode.
            eprintln!("sett: couldn't save settings to {}: {err}", path.display());
        }
    }
}

/// The local calendar date, which decides when a Task was done and whether
/// it is overdue.
pub(crate) fn today() -> Date {
    use chrono::Datelike as _;
    let now = chrono::Local::now().date_naive();
    let date = || {
        Date::new(
            u16::try_from(now.year()).ok()?,
            u8::try_from(now.month()).ok()?,
            u8::try_from(now.day()).ok()?,
        )
    };
    date().expect("the local date is a valid calendar date")
}

fn note_change(project: Option<&str>, slug: &str) -> Change {
    Change::Note {
        project: project.map(str::to_string),
        slug: slug.to_string(),
    }
}

fn task_change(project: &str, slug: &str) -> Change {
    Change::Task {
        project: project.to_string(),
        slug: slug.to_string(),
    }
}

/// Starts watching `space` and applies each outside change to the index on
/// the foreground. Without a watcher the Space still works; it just won't
/// pick up outside edits until it is reopened.
fn watch(store: &Store, space: Space, cx: &mut Context<AppModel>) -> Option<Watching> {
    let watcher = match store.watch(space) {
        Ok(watcher) => watcher,
        Err(err) => {
            eprintln!("sett: couldn't watch {}: {err}", space.dir_name());
            return None;
        }
    };
    let events = watcher.events();
    let task = cx.spawn(async move |this, cx| {
        while let Ok(change) = events.recv().await {
            let applied = this.update(cx, |model, cx| {
                if model.space() == space {
                    model.apply_change(change, cx);
                }
            });
            if applied.is_err() {
                break;
            }
        }
    });
    Some(Watching {
        _watcher: watcher,
        _task: task,
    })
}

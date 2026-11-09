use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use gpui_kit::{App, AppContext as _, Context, Entity, Task};
use sett_store::{
    Board, Change, Date, INBOX_SLUG, Project, Settings, Space, SpaceIndex, Store, Update, Watcher,
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

/// The open data root and the active Space: its index, kept current by a
/// watcher, and each Space's last selection. Only the active Space is indexed
/// and watched, so nothing from the other Space is held (ADR 0002).
pub struct Workspace {
    store: Store,
    settings: Settings,
    settings_path: Option<PathBuf>,
    index: SpaceIndex,
    selections: HashMap<Space, Selection>,
    watched: bool,
    watching: Option<Watching>,
}

struct Watching {
    _watcher: Watcher,
    _task: Task<()>,
}

impl Workspace {
    /// Opens the data root from `settings` (running first-run setup if
    /// needed) in the last used Space. `settings_path` is where the last
    /// Space is saved; `None` skips saving.
    pub fn open(
        settings: Settings,
        settings_path: Option<PathBuf>,
        cx: &mut App,
    ) -> io::Result<Entity<Workspace>> {
        Workspace::open_with(settings, settings_path, true, cx)
    }

    /// Like [`Workspace::open`] but without watching the data root, for when
    /// the caller reports outside changes itself through
    /// [`Workspace::apply_change`]. GPUI's test executor rejects wakeups from
    /// the watcher's thread, so UI tests use this.
    pub fn open_unwatched(
        settings: Settings,
        settings_path: Option<PathBuf>,
        cx: &mut App,
    ) -> io::Result<Entity<Workspace>> {
        Workspace::open_with(settings, settings_path, false, cx)
    }

    fn open_with(
        settings: Settings,
        settings_path: Option<PathBuf>,
        watched: bool,
        cx: &mut App,
    ) -> io::Result<Entity<Workspace>> {
        let store = Store::open(&settings.data_root)?;
        Ok(cx.new(|cx| {
            let space = settings.last_space;
            let watching = watched.then(|| watch(&store, space, cx)).flatten();
            let index = SpaceIndex::scan(&store, space);
            Workspace {
                store,
                settings,
                settings_path,
                index,
                selections: HashMap::new(),
                watched,
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

    pub fn switch_space(&mut self, space: Space, cx: &mut Context<Self>) {
        if space == self.space() {
            return;
        }
        // Drop the old watcher first so no late event from it reaches the new index.
        self.watching = None;
        if self.watched {
            self.watching = watch(&self.store, space, cx);
        }
        self.index = SpaceIndex::scan(&self.store, space);
        self.fix_selection();
        self.save_last_space(space);
        cx.notify();
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
        self.reload(&[task_change(project, &task.slug)], cx);
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
        let columns = self.project_columns(project)?;
        let board = Board::new(&columns, self.index.tasks(project));
        let written = self
            .store
            .move_task(&board, slug, column, before, today())?;
        let changes: Vec<Change> = written
            .iter()
            .map(|t| task_change(project, &t.slug))
            .collect();
        self.reload(&changes, cx);
        Ok(())
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

    fn project_columns(&self, project: &str) -> io::Result<Vec<String>> {
        self.index
            .project(project)
            .map(|p| p.columns.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "project not found"))
    }

    /// Runs a Column edit, then reloads the whole Project, since an edit can
    /// rewrite any of its Tasks.
    fn edit_columns(
        &mut self,
        slug: &str,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&Store, &mut Project, Vec<&sett_store::Task>) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut project = self
            .index
            .project(slug)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "project not found"))?;
        let result = edit(&self.store, &mut project, self.index.tasks(slug));
        // Reload even on failure: an edit can fail after writing some files.
        self.reload(&[Change::Project(slug.to_string())], cx);
        result
    }

    /// sett's own writes never come back through the watcher, so the index
    /// is brought up to date here instead.
    fn reload(&mut self, changes: &[Change], cx: &mut Context<Self>) {
        for change in changes {
            self.index.apply(&self.store, change);
        }
        cx.notify();
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
        if let Some(path) = &self.settings_path
            && let Err(err) = self.settings.save(path)
        {
            // Not worth interrupting the user: the worst case is reopening in
            // the other Space.
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

fn task_change(project: &str, slug: &str) -> Change {
    Change::Task {
        project: project.to_string(),
        slug: slug.to_string(),
    }
}

/// Starts watching `space` and applies each outside change to the index on
/// the foreground. Without a watcher the Space still works; it just won't
/// pick up outside edits until it is reopened.
fn watch(store: &Store, space: Space, cx: &mut Context<Workspace>) -> Option<Watching> {
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
            let applied = this.update(cx, |workspace, cx| {
                if workspace.space() == space {
                    workspace.apply_change(change, cx);
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

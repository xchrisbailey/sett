use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use gpui_kit::{App, AppContext as _, Context, Entity, Task};
use sett_store::{Change, INBOX_SLUG, Settings, Space, SpaceIndex, Store, Update, Watcher};

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

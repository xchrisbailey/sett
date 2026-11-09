use std::io;
use std::time::Duration;

use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Render, Role, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
    div,
};
use markdown_editor::{BlockEditor, BlockEditorEvent, BlockEditorState};
use sett_store::{Date, EditorMode, JournalEntry, Note, Space};

use crate::workspace::Workspace;

/// How long after the last keystroke an edit is written.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(500);

/// The Item an editor has open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemRef {
    Note {
        space: Space,
        project: Option<String>,
        slug: String,
    },
    Journal {
        space: Space,
        date: Date,
    },
    /// The Task view shows the title itself, above the Task's fields.
    Task {
        space: Space,
        project: String,
        slug: String,
    },
}

impl ItemRef {
    pub(crate) fn note(space: Space, project: Option<&str>, slug: &str) -> ItemRef {
        ItemRef::Note {
            space,
            project: project.map(str::to_string),
            slug: slug.to_string(),
        }
    }

    fn space(&self) -> Space {
        match self {
            ItemRef::Note { space, .. }
            | ItemRef::Journal { space, .. }
            | ItemRef::Task { space, .. } => *space,
        }
    }
}

/// An Item as read from the index.
#[derive(Debug, Clone)]
enum Version {
    Note(Note),
    Journal(JournalEntry),
    Task(sett_store::Task),
}

impl Version {
    fn title(&self) -> Option<&str> {
        match self {
            Version::Note(note) => Some(&note.title),
            Version::Journal(_) => None,
            Version::Task(task) => Some(&task.title),
        }
    }

    fn body(&self) -> &str {
        match self {
            Version::Note(note) => &note.body,
            Version::Journal(entry) => &entry.body,
            Version::Task(task) => &task.body,
        }
    }

    /// Only the text the editor shows counts: a new mtime or a hand-edited
    /// frontmatter key isn't something the user needs to choose about.
    fn same_text(&self, other: &Version) -> bool {
        self.title() == other.title() && self.body() == other.body()
    }
}

/// Edits one Note or Journal Entry: an editable title (Notes only) above the
/// body in the block view or as raw Markdown, following the global mode.
///
/// The body is the only part the editor sees; frontmatter stays with
/// sett-store. The file is written only after a real edit (ADR 0003): 500 ms
/// after the last one, when another Item is opened, or when sett quits.
pub(crate) struct ItemEditor {
    workspace: Entity<Workspace>,
    item: Option<ItemRef>,
    /// The file as last read or written, or `None` when there is no file
    /// (yet). An index version that differs from this changed outside sett.
    seen: Option<Version>,
    /// What a save starts from, so frontmatter keys the editor doesn't know
    /// about survive.
    base: Option<Version>,
    /// The body as the user last left it, in whichever mode.
    body: String,
    dirty: bool,
    /// The keep-mine/load-theirs prompt is open; nothing is saved meanwhile.
    asking: bool,
    mode: EditorMode,
    title: Entity<InputState>,
    block: Entity<BlockEditorState>,
    raw: Entity<EditorState>,
    scroll: ScrollHandle,
    autosave: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ItemEditor {
    pub(crate) fn new(
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let scroll = ScrollHandle::new();
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Untitled"));
        let block = cx.new(|cx| {
            let mut state = BlockEditorState::new(window, cx);
            state.set_scroll_handle(scroll.clone());
            state
        });
        let raw = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .soft_wrap(true)
        });
        let subscriptions = vec![
            cx.subscribe_in(&block, window, |this, block, event, window, cx| {
                if let BlockEditorEvent::Changed { .. } = event {
                    this.body = block.read(cx).source();
                    this.edited(window, cx);
                }
            }),
            cx.subscribe_in(&raw, window, |this, raw, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    this.body = raw.read(cx).value().to_string();
                    this.edited(window, cx);
                }
            }),
            cx.subscribe_in(&title, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    this.edited(window, cx);
                }
            }),
            cx.observe_in(&workspace, window, |this, _, window, cx| {
                this.sync(window, cx)
            }),
            cx.on_app_quit(|this, cx| {
                if let Err(err) = this.save(cx) {
                    eprintln!("sett: couldn't save on quit: {err}");
                }
                async {}
            }),
        ];
        let mode = workspace.read(cx).editor_mode();
        ItemEditor {
            workspace,
            item: None,
            seen: None,
            base: None,
            body: String::new(),
            dirty: false,
            asking: false,
            mode,
            title,
            block,
            raw,
            scroll,
            autosave: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn item(&self) -> Option<&ItemRef> {
        self.item.as_ref()
    }

    /// The title box, for a host that lays the title out itself.
    pub(crate) fn title_input(&self) -> &Entity<InputState> {
        &self.title
    }

    /// Opens `item`, saving the one open before if it has edits. A Journal
    /// Entry without a file opens empty and stays in memory until typed in.
    pub(crate) fn open(&mut self, item: ItemRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.item.as_ref() == Some(&item) {
            return;
        }
        self.flush(window, cx);
        self.mode = self.workspace.read(cx).editor_mode();
        self.item = Some(item);
        let disk = self.disk_version(cx);
        match disk.clone().or_else(|| self.empty_version()) {
            Some(base) => self.show(disk, base, window, cx),
            None => self.close(cx),
        }
    }

    /// Focuses the title with its text selected, for a Note just created.
    pub(crate) fn focus_title(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.title.update(cx, |title, cx| {
            title.focus_handle(cx).focus(window, cx);
            title.select_all(window, cx);
        });
    }

    /// Writes pending edits now, reporting a failure.
    pub(crate) fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = self.save(cx) {
            report(err, window, cx);
        }
    }

    /// Closes the Item without saving, for when it is being deleted.
    pub(crate) fn discard(&mut self, cx: &mut Context<Self>) {
        self.close(cx);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.item = None;
        self.seen = None;
        self.base = None;
        self.dirty = false;
        self.asking = false;
        self.autosave = None;
        cx.notify();
    }

    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.item.is_none() {
            return;
        }
        self.dirty = true;
        let timer = cx.background_executor().timer(AUTOSAVE_DELAY);
        self.autosave = Some(cx.spawn_in(window, async move |this, cx| {
            timer.await;
            this.update_in(cx, |this, window, cx| this.flush(window, cx))
                .ok();
        }));
    }

    fn save(&mut self, cx: &mut Context<Self>) -> io::Result<()> {
        if !self.dirty || self.asking {
            return Ok(());
        }
        let Some(base) = self.disk_version(cx).or_else(|| self.base.clone()) else {
            return Ok(());
        };
        let workspace = self.workspace.clone();
        let saved = match base {
            Version::Note(mut note) => {
                note.title = self.title_or_untitled(cx);
                note.body = self.body.clone();
                workspace.update(cx, |w, cx| w.save_note(&mut note, cx))?;
                Version::Note(note)
            }
            Version::Journal(mut entry) => {
                entry.body = self.body.clone();
                workspace.update(cx, |w, cx| w.save_journal_entry(&mut entry, cx))?;
                Version::Journal(entry)
            }
            Version::Task(mut task) => {
                task.title = self.title_or_untitled(cx);
                task.body = self.body.clone();
                workspace.update(cx, |w, cx| w.save_task(&mut task, cx))?;
                Version::Task(task)
            }
        };
        self.seen = Some(saved.clone());
        self.base = Some(saved);
        self.dirty = false;
        Ok(())
    }

    fn title_or_untitled(&self, cx: &App) -> String {
        let title = self.title.read(cx).value();
        let title = title.trim();
        if title.is_empty() {
            "Untitled".to_string()
        } else {
            title.to_string()
        }
    }

    /// Follows the Workspace: the editor mode, a Space switch, and outside
    /// changes to the open Item.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item.clone() else {
            return;
        };
        let workspace = self.workspace.read(cx);
        if workspace.space() != item.space() {
            self.flush(window, cx);
            self.close(cx);
            return;
        }
        let mode = workspace.editor_mode();
        if mode != self.mode {
            self.set_mode(mode, window, cx);
        }
        if self.asking {
            return;
        }
        let disk = self.disk_version(cx);
        match (&self.seen, &disk) {
            (Some(seen), Some(now)) if seen.same_text(now) => {
                self.seen = disk.clone();
                self.base = disk;
            }
            (None, None) => {}
            _ if self.dirty => self.ask_keep_or_load(window, cx),
            _ => self.load_disk(window, cx),
        }
    }

    /// Shows what is on disk now, or closes a Note that is gone.
    fn load_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let disk = self.disk_version(cx);
        match disk.clone().or_else(|| self.empty_version()) {
            Some(base) => self.show(disk, base, window, cx),
            None => self.close(cx),
        }
    }

    fn ask_keep_or_load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.asking = true;
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (keep, load) = (this.clone(), this.clone());
            alert
                .confirm()
                .keyboard(false)
                .title("Changed outside sett")
                .description(
                    "This file changed on disk while you had unsaved edits. \
                     Keep yours and overwrite the file, or load the version on disk?",
                )
                .ok_text("Load theirs")
                .cancel_text("Keep mine")
                .on_ok(move |_, window, cx| {
                    load.update(cx, |this, cx| this.resolve(false, window, cx))
                        .ok();
                    true
                })
                .on_cancel(move |_, window, cx| {
                    keep.update(cx, |this, cx| this.resolve(true, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn resolve(&mut self, keep_mine: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.asking = false;
        if keep_mine {
            let disk = self.disk_version(cx);
            if disk.is_some() {
                self.base = disk.clone();
            }
            self.seen = disk;
            self.edited(window, cx);
        } else {
            self.load_disk(window, cx);
        }
    }

    fn show(
        &mut self,
        seen: Option<Version>,
        base: Version,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body = base.body().to_string();
        let title = base.title().unwrap_or_default().to_string();
        self.title
            .update(cx, |input, cx| input.set_value(title, window, cx));
        self.load_body(window, cx);
        self.seen = seen;
        self.base = Some(base);
        self.dirty = false;
        self.asking = false;
        self.autosave = None;
        cx.notify();
    }

    fn set_mode(&mut self, mode: EditorMode, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.active_focus_handle(cx).contains_focused(window, cx);
        self.mode = mode;
        self.load_body(window, cx);
        if focused {
            self.active_focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    fn active_focus_handle(&self, cx: &App) -> gpui_kit::FocusHandle {
        match self.mode {
            EditorMode::Block => self.block.focus_handle(cx),
            EditorMode::Raw => self.raw.focus_handle(cx),
        }
    }

    /// Puts the body into the editor for the current mode. Neither editor
    /// reports this as an edit.
    fn load_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.body.clone();
        match self.mode {
            EditorMode::Block => {
                let result = self
                    .block
                    .update(cx, |block, cx| block.set_source(&body, cx));
                if let Err(err) = result {
                    eprintln!("sett: couldn't show the block view: {err}");
                }
            }
            EditorMode::Raw => self
                .raw
                .update(cx, |raw, cx| raw.set_value(body, window, cx)),
        }
    }

    fn disk_version(&self, cx: &App) -> Option<Version> {
        let workspace = self.workspace.read(cx);
        let item = self.item.as_ref()?;
        if workspace.space() != item.space() {
            return None;
        }
        let index = workspace.index();
        match item {
            ItemRef::Note { project, slug, .. } => index
                .note(project.as_deref(), slug)
                .cloned()
                .map(Version::Note),
            ItemRef::Journal { date, .. } => {
                index.journal_entry(*date).cloned().map(Version::Journal)
            }
            ItemRef::Task { project, slug, .. } => {
                index.task(project, slug).cloned().map(Version::Task)
            }
        }
    }

    /// A Journal Entry with no file is simply empty; a Note or Task must exist.
    fn empty_version(&self) -> Option<Version> {
        match self.item.as_ref()? {
            ItemRef::Journal { space, date } => {
                Some(Version::Journal(JournalEntry::new(*space, *date)))
            }
            ItemRef::Note { .. } | ItemRef::Task { .. } => None,
        }
    }

    fn focus_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.block.update(cx, |block, cx| {
            block.focus(window, cx);
            // Only fails for an empty document, where the caret is already
            // at the end.
            block.select_end(cx).ok();
        });
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let label = match self.mode {
            EditorMode::Block => "Block editor",
            EditorMode::Raw => "Raw Markdown",
        };
        let body = div()
            .id("note-body")
            .role(Role::Group)
            .test_support()
            .aria_label(label)
            .flex_1()
            .min_h_0();
        match self.mode {
            // The editor is only as tall as its text (the fork styles an inner
            // node), so the space below it puts the caret at the end.
            EditorMode::Block => body
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(BlockEditor::new(&self.block).px_8().pt_4())
                .child(div().flex_grow(1.).min_h_16().cursor_text().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus_end(window, cx);
                        // Otherwise the shell takes focus back.
                        window.prevent_default();
                    }),
                ))
                .into_any_element(),
            EditorMode::Raw => body
                .flex()
                .flex_col()
                .px_6()
                .py_2()
                .child(Editor::new(&self.raw).appearance(false).size_full())
                .into_any_element(),
        }
    }
}

/// A long local date such as "Monday, 9 November 2026".
fn format_date(date: Date) -> String {
    chrono::NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::from(date.month()),
        u32::from(date.day()),
    )
    .map(|d| d.format("%A, %-d %B %Y").to_string())
    .unwrap_or_else(|| date.to_string())
}

fn report(err: io::Error, window: &mut Window, cx: &mut App) {
    window.push_notification(
        Notification::error(format!("Couldn\u{2019}t save your edits. {err}.")),
        cx,
    );
}

impl Render for ItemEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(item) = &self.item else {
            return div().into_any_element();
        };
        let header = match item {
            ItemRef::Note { .. } => div().px_6().pt_4().child(
                Input::new(&self.title)
                    .id("note-title")
                    .appearance(false)
                    .large()
                    .text_xl()
                    .font_semibold(),
            ),
            ItemRef::Journal { date, .. } => div()
                .px_8()
                .pt_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format_date(*date)),
            ItemRef::Task { .. } => div(),
        };
        v_flex()
            .size_full()
            .min_h_0()
            .child(header)
            .child(self.render_body(cx))
            .into_any_element()
    }
}

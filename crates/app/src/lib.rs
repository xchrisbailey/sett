//! The sett desktop app: the window shell, the left nav and the center panel,
//! built on `sett-store`.

mod board;
mod center;
mod icons;
mod item_editor;
mod nav;
mod notes;
mod project_dialog;
mod shell;
mod task_view;
mod workspace;

pub use board::BoardEvent;
pub use icons::AppAssets;
pub use shell::{
    ArchiveProject, DeleteProject, EditProject, NewProject, Shell, ShowPersonal, ShowWork,
    ToggleEditorMode, UnarchiveProject,
};
pub use workspace::{Selection, Workspace};

/// Registers the app's key bindings. Call after `gpui_kit::init` and before
/// `set_menus`.
pub fn init(cx: &mut gpui_kit::App) {
    markdown_editor::init(cx);
    board::init(cx);
    shell::init(cx);
    task_view::init(cx);
}

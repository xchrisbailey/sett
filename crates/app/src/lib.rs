//! The sett desktop app: the window shell, the left nav and the center panel,
//! built on `sett-store`.

mod app_model;
mod board;
mod center;
mod icons;
mod item_editor;
mod item_links;
mod nav;
mod notes;
mod notice;
mod project_dialog;
mod shell;
mod task_view;

pub use app_model::{AppModel, Selection};
pub use board::BoardEvent;
pub use icons::AppAssets;
pub use shell::{
    ArchiveProject, DeleteProject, EditProject, NewProject, Shell, ShowPersonal, ShowWork,
    ToggleEditorMode, UnarchiveProject,
};

/// Registers the app's key bindings. Call after `gpui_kit::init` and before
/// `set_menus`.
pub fn init(cx: &mut gpui_kit::App) {
    markdown_editor::init(cx);
    board::init(cx);
    shell::init(cx);
    task_view::init(cx);
}

//! The sett desktop app: the window shell, the left nav and the center panel,
//! built on `sett-store`.

mod app_model;
mod board;
mod center;
mod confirm_delete;
mod dates;
mod icons;
mod item_editor;
mod item_links;
mod label_field;
mod labels;
mod link_data;
mod nav;
mod notes;
mod notice;
mod project_dialog;
mod shell;
mod task_view;
mod template_options;
mod templates;

pub use app_model::{AppModel, AppModelEvent, Selection};
pub use board::BoardEvent;
pub use icons::AppAssets;
pub use shell::{
    ArchiveProject, DeleteProject, EditProject, GoBack, GoForward, NewProject, Shell, ShowPersonal,
    ShowWork, ToggleEditorMode, UnarchiveProject,
};

/// Registers the app's key bindings. Call after `gpui_kit::init` and before
/// `set_menus`.
/// For `scripts/screenshot --insert-menu`: for the rest of this launch, a body
/// the editor loads that ends in a `/query` line opens its Insert Menu.
pub fn open_insert_menu_on_load(cx: &mut gpui_kit::App) {
    cx.set_global(item_editor::OpenInsertMenuOnLoad);
}

pub fn init(cx: &mut gpui_kit::App) {
    markdown_editor::init(cx);
    board::init(cx);
    notes::init(cx);
    shell::init(cx);
    task_view::init(cx);
    templates::init(cx);
}

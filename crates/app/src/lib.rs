//! The sett desktop app: the window shell, the left nav and the center panel,
//! built on `sett-store`.

mod board;
mod center;
mod icons;
mod nav;
mod project_dialog;
mod shell;
mod workspace;

pub use board::BoardEvent;
pub use icons::AppAssets;
pub use shell::{
    ArchiveProject, DeleteProject, EditProject, NewProject, Shell, ShowPersonal, ShowWork,
    UnarchiveProject,
};
pub use workspace::{Selection, Workspace};

/// Registers the app's key bindings. Call after `gpui_kit::init` and before
/// `set_menus`.
pub fn init(cx: &mut gpui_kit::App) {
    shell::init(cx);
}

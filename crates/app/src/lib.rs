//! The sett desktop app: the window shell, the left nav and the center panel,
//! built on `sett-store`.

mod board;
mod center;
mod nav;
mod shell;
mod workspace;

pub use board::BoardEvent;
pub use shell::{Shell, ShowPersonal, ShowWork};
pub use workspace::{Selection, Workspace};

/// Registers the app's key bindings. Call after `gpui_kit::init` and before
/// `set_menus`.
pub fn init(cx: &mut gpui_kit::App) {
    shell::init(cx);
}

use std::rc::Rc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::{App, SharedString, Window};

/// Asks before moving `name` to the Trash, running `delete` only once the
/// user confirms. `consequence` says what goes and what stops working.
pub(crate) fn confirm_delete(
    name: &str,
    consequence: &'static str,
    window: &mut Window,
    cx: &mut App,
    delete: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = SharedString::from(format!("Delete \u{201c}{name}\u{201d}?"));
    let delete = Rc::new(delete);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let delete = delete.clone();
        alert
            .confirm()
            .title(title.clone())
            .description(consequence)
            .ok_text("Delete")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, window, cx| {
                delete(window, cx);
                true
            })
    });
}

/// What deleting a Template does.
pub(crate) const TEMPLATE_CONSEQUENCE: &str = "Tasks and Notes made from it aren\u{2019}t changed.";

/// What deleting a Note or Task does.
pub(crate) const ITEM_CONSEQUENCE: &str =
    "The file moves to the Trash. Links to it from other Items will stop working.";

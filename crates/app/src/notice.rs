use gpui_kit::component::alert::Alert;
use gpui_kit::{
    InteractiveElement as _, IntoElement, ParentElement as _, TestSupportExt as _, div,
};

/// Shown on an Item or Project whose frontmatter can't be read. sett never
/// writes such a file (ADR 0001), so editing it here is disabled.
pub(crate) fn frontmatter_notice() -> impl IntoElement {
    div().id("frontmatter-error").test_support().child(Alert::warning(
        "frontmatter-error-alert",
        "Frontmatter can\u{2019}t be read \u{2014} fix it in an editor; sett won\u{2019}t save changes.",
    ))
}

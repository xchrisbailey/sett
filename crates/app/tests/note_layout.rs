//! How a Note's body lays out when the window is narrow (#49).

mod common;

use common::{note, open_shell_sized, settle, write};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Pixels, TestAppContext, px};
use sett::Selection;

/// The Budget Note from the screenshot fixtures. The Link chip opens the
/// paragraph after the table, so its top tracks the table's bottom.
const BUDGET: &str = "---\ntitle: Budget\n---\n\
| Item     | Estimate |\n\
|----------|----------|\n\
| Cabinets | 6,000    |\n\
| Worktop  | 2,500    |\n\
\n\
[[budget]] tracks the quotes.\n";

/// How far below the top of the editor the paragraph after the Budget
/// table starts, in a window `width` points wide.
fn below_table(cx: &mut TestAppContext, width: f32) -> Pixels {
    let f = open_shell_sized(cx, width, 919.);
    write(cx, &f, "notes/budget.md", BUDGET, note(None, "budget"));
    let model = f.model(cx);
    cx.update(|cx| model.update(cx, |w, cx| w.select(Selection::Notes, cx)));
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("note-budget", cx);
    })
    .unwrap();
    settle(cx);
    cx.update_window(f.window, |_, window, cx| {
        window.render_frame(cx);
        let body = window.find("note-body").bounds();
        let chip = window.within("note-body").find_by_label("Budget").bounds();
        chip.top() - body.top()
    })
    .unwrap()
}

/// In a half-screen window the Note editor is narrow. A table whose cells
/// each fit on one line in a wide window must not wrap inside a word there
/// ("Estimat / e"): its rows keep their one-line height, so what follows the
/// table doesn't move down.
#[gpui_kit::test]
fn a_table_keeps_whole_words_in_a_half_screen_window(cx: &mut TestAppContext) {
    let wide = below_table(cx, 2000.);
    let half = below_table(cx, 743.);
    assert!(
        (half - wide).abs() < px(1.),
        "the table ends {half:?} down at 743pt but {wide:?} at 2000pt: its cells wrapped"
    );
}

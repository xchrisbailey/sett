//! Copy and paste at a Code Block's edges (#68), at the model level: the
//! same `copy_selection` and `paste_clipboard` the editor's Copy and Paste
//! run, minus GPUI.

use super::*;
use crate::state::CopyKind;

const DOC: &str = "Intro\n\n```rust title=x\nfn a() {}\nlet b = 1;\n```\n\nOutro";

fn paste(m: &mut BlockEditorModel, text: &str, plain: bool, copy: &CopyKind) {
    m.paste_clipboard(text, plain, copy).unwrap();
}

/// The text a partial copy inside the first Code Block of `DOC` puts on the
/// clipboard, with its kind.
fn copy_inside(range: std::ops::Range<usize>) -> (String, CopyKind) {
    let mut m = model(DOC);
    select(&mut m, 1, range);
    m.copy_selection()
}

#[test]
fn whole_block_copy_keeps_lang_and_meta() {
    let mut m = model(DOC);
    let id = m.document().leaves().nth(1).unwrap().id();
    m.set_selection(BlockSelection::Blocks {
        anchor: id,
        head: id,
    })
    .unwrap();
    let (text, kind) = m.copy_selection();
    assert_eq!(text, "```rust title=x\nfn a() {}\nlet b = 1;\n```");
    assert_eq!(kind, CopyKind::Blocks);
}

#[test]
fn copy_crossing_into_the_block_keeps_lang_and_meta() {
    let mut m = model(DOC);
    let code = m.document().leaves().nth(1).unwrap().id();
    let para = m.document().leaves().next().unwrap().id();
    m.set_selection(BlockSelection::Text {
        anchor: BlockPosition::new(para, 2),
        head: BlockPosition::new(code, 4),
    })
    .unwrap();
    let (text, kind) = m.copy_selection();
    assert_eq!(kind, CopyKind::Blocks);
    assert!(text.contains("```rust title=x\nfn a"), "{text}");
}

#[test]
fn fenced_paste_into_paragraph_keeps_lang_and_meta() {
    let mut m = model("");
    paste(
        &mut m,
        "```rust title=x\nfn a() {}\n```",
        false,
        &CopyKind::External,
    );
    assert_eq!(m.source(), "```rust title=x\nfn a() {}\n```");
}

#[test]
fn copy_inside_a_block_is_raw_code_tagged_with_lang_and_meta() {
    let (text, kind) = copy_inside(0..9);
    assert_eq!(text, "fn a() {}");
    let CopyKind::Code { lang, meta } = &kind else {
        panic!("{kind:?}")
    };
    assert_eq!(lang.as_deref(), Some("rust"));
    assert_eq!(meta.as_deref(), Some("title=x"));
    // The clipboard tag round-trips the info string.
    let tag = kind.tag().unwrap();
    assert_eq!(CopyKind::from_tag(&tag), kind);
}

#[test]
fn tag_of_a_bare_block_round_trips() {
    let kind = CopyKind::Code {
        lang: None,
        meta: None,
    };
    assert_eq!(CopyKind::from_tag(&kind.tag().unwrap()), kind);
}

#[test]
fn code_copy_one_line_pastes_as_text_in_a_paragraph() {
    let (_, kind) = copy_inside(0..9);
    let mut m = model("");
    paste(&mut m, "# comment", false, &kind);
    assert_eq!(m.source(), "\\# comment");
    assert_eq!(m.document().leaves().count(), 1);
}

#[test]
fn code_copy_several_lines_pastes_as_a_tagged_code_block() {
    let (text, kind) = copy_inside(0..20);
    let text = format!("# one\n{text}\nthree");
    let mut m = model("");
    paste(&mut m, &text, false, &kind);
    assert_eq!(
        m.source(),
        "```rust title=x\n# one\nfn a() {}\nlet b = 1;\nthree\n```"
    );
}

#[test]
fn code_copy_into_a_code_block_is_raw() {
    let (_, kind) = copy_inside(0..9);
    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, "# **x**", false, &kind);
    assert_eq!(m.source(), "```python\n# **x**\n```");
}

fn whole_block() -> (String, CopyKind) {
    (
        "```rust title=x\nfn a() {}\n```".to_string(),
        CopyKind::Blocks,
    )
}

#[test]
fn fenced_paste_into_a_code_block_drops_the_fences_and_keeps_its_language() {
    let (text, kind) = whole_block();
    let mut m = model("```python\nx\n```");
    caret_at_end(&mut m, 0);
    paste(&mut m, &text, false, &kind);
    assert_eq!(m.source(), "```python\nxfn a() {}\n```");
}

#[test]
fn a_code_copy_that_looks_like_a_fence_pastes_raw_into_a_code_block() {
    let kind = CopyKind::Code {
        lang: None,
        meta: None,
    };
    let text = "```js\nlet a;\n```";
    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, text, false, &kind);
    assert_eq!(
        m.source().trim_end(),
        crate::serialize::fenced_code(Some("python"), None, text)
    );
}

#[test]
fn tilde_fence_paste_is_stripped_too() {
    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, "~~~js\nlet a;\n~~~\n", false, &CopyKind::External);
    assert_eq!(m.source(), "```python\nlet a;\n```");
}

#[test]
fn fenced_paste_into_a_bare_block_takes_lang_and_meta_in_one_undo_step() {
    let (text, kind) = whole_block();
    let mut m = model("```\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, &text, false, &kind);
    assert_eq!(m.source(), "```rust title=x\nfn a() {}\n```");
    assert!(m.undo());
    assert_eq!(m.source(), "```\n\n```");
}

#[test]
fn two_fences_or_prose_are_inserted_raw() {
    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(
        &mut m,
        "```a\nx\n```\n```b\ny\n```",
        false,
        &CopyKind::External,
    );
    assert!(
        m.source().contains("```a\nx\n```\n```b\ny\n```"),
        "{}",
        m.source()
    );

    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, "see\n```a\nx\n```", false, &CopyKind::External);
    assert_eq!(m.source(), "````python\nsee\n```a\nx\n```\n````");
}

#[test]
fn plain_paste_of_a_fence_is_inserted_raw() {
    let mut m = model("```python\n```");
    caret_in(&mut m, 0, 0);
    paste(&mut m, "```a\nx\n```", true, &CopyKind::External);
    assert_eq!(m.source(), "````python\n```a\nx\n```\n````");
}

use sett_store::{CalloutKind, Date, Space, Store, TaskCallout};

use CalloutKind::{Caution, Important, Note, Tip, Warning};

/// A Task body and the Callout the Card should show for it.
type Row = (&'static str, Option<(CalloutKind, &'static [&'static str])>);

fn first(body: &str) -> Option<TaskCallout> {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("sett")).unwrap();
    let mut task = store
        .create_task(
            Space::Work,
            "inbox",
            "t",
            Date::parse("2026-03-04").unwrap(),
        )
        .unwrap();
    task.body = body.to_string();
    task.first_callout()
}

fn check(rows: &[Row]) {
    for (body, expected) in rows {
        let expected = expected.map(|(kind, lines)| TaskCallout {
            kind,
            lines: lines.iter().map(|line| line.to_string()).collect(),
        });
        assert_eq!(first(body), expected, "body: {body:?}");
    }
}

/// Which quote counts as a Callout, and which of several wins. The same
/// bodies are in the editor-parity samples in `crates/app/tests`.
#[test]
fn finds_the_first_top_level_callout_with_text() {
    check(&[
        ("", None),
        ("# Title\n\nJust text.\n", None),
        ("> a plain quote\n", None),
        (
            "Intro\n\n- a\n- b\n\n> plain\n\n> [!TIP]\n> Use it.\n",
            Some((Tip, &["Use it."])),
        ),
        ("> [!NOTE]\n> x\n", Some((Note, &["x"]))),
        ("> [!tip]\n> x\n", Some((Tip, &["x"]))),
        ("> [!Important]\n> x\n", Some((Important, &["x"]))),
        ("> [!wArNiNg]\n> x\n", Some((Warning, &["x"]))),
        ("> [!CAUTION]\n> x\n", Some((Caution, &["x"]))),
        ("> [!FOO]\n> x\n", None),
        ("   > [!NOTE]\n   > x\n", Some((Note, &["x"]))),
        ("> [!NOTE] inline\n> x\n", None),
        ("> \\[!NOTE]\n> x\n", None),
        (
            "> [!WARNING]\r\n> careful\r\n> now\r\n",
            Some((Warning, &["careful", "now"])),
        ),
        (
            "> [!NOTE]\n> one\n\n> [!TIP]\n> two\n",
            Some((Note, &["one"])),
        ),
        // Empty Callouts are skipped.
        ("> [!NOTE]\n", None),
        ("> [!NOTE]\n>\n>  \n", None),
        ("> [!NOTE]\n> ```\n> ```\n", None),
        (
            "> [!NOTE]\n\ntext\n\n> [!TIP]\n> second\n",
            Some((Tip, &["second"])),
        ),
        (
            "> [!NOTE]\n>\n\n> [!CAUTION]\n> second\n",
            Some((Caution, &["second"])),
        ),
    ]);
}

/// Where a quote can't be a Callout: inside another block.
#[test]
fn quotes_inside_other_blocks_are_not_callouts() {
    check(&[
        // List items, including through a lazy continuation line.
        ("- item\n  > [!NOTE]\n  > x\n", None),
        ("1. item\n   > [!NOTE]\n   > x\n", None),
        ("- item\nlazy\n  > [!NOTE]\n  > x\n", None),
        (
            "- item\n  > [!NOTE]\n  > in\n\n> [!TIP]\n> out\n",
            Some((Tip, &["out"])),
        ),
        // A quote right after an item is not lazy: it interrupts the item.
        ("- item\n> [!NOTE]\n> x\n", Some((Note, &["x"]))),
        // Quotes.
        ("> > [!NOTE]\n> > x\n", None),
        ("> text\n> [!NOTE]\n> x\n", None),
        // Code.
        ("```\n> [!NOTE]\n> x\n```\n", None),
        ("~~~\n> [!NOTE]\n> x\n~~~\n", None),
        ("```\n> [!NOTE]\n> x\n", None),
        (
            "```\n> [!NOTE]\n> x\n```\n> [!TIP]\n> y\n",
            Some((Tip, &["y"])),
        ),
        ("para\n\n    > [!NOTE]\n    > x\n", None),
    ]);
}

/// Where a Callout's text ends, and which source lines it keeps.
#[test]
fn the_callout_ends_at_a_blank_line_or_a_new_block() {
    check(&[
        ("> [!NOTE]\nlazy\n\nafter\n", Some((Note, &["lazy"]))),
        (
            "> [!NOTE]\n> one\ntwo\n\nafter\n",
            Some((Note, &["one", "two"])),
        ),
        ("> [!NOTE]\n> a\n\n> b\n", Some((Note, &["a"]))),
        ("> [!NOTE]\n> a\n>\n> b\n", Some((Note, &["a", "b"]))),
    ]);
}

/// Markdown markup in a Callout turned into plain text.
#[test]
fn inline_markup_becomes_plain_text() {
    check(&[
        (
            "> [!NOTE]\n> **bold** __also__ *it* _em_ `code` snake_case\n",
            Some((Note, &["bold also it em code snake_case"])),
        ),
        // Only emphasis stars go; a star that can't open and close stays.
        (
            "> [!NOTE]\n> 2 * 3 and a*b*c\n",
            Some((Note, &["2 * 3 and abc"])),
        ),
        ("> [!NOTE]\n> **open only\n", Some((Note, &["**open only"]))),
        (
            "> [!NOTE]\n> see [the **docs**](https://x.y/z) now\n",
            Some((Note, &["see the docs now"])),
        ),
        (
            "> [!NOTE]\n> [[plan|the plan]] [[plan]] [[plan#Heading]]\n",
            Some((Note, &["the plan plan plan"])),
        ),
        (
            "> [!NOTE]\n> 2 \\* 3 \\[x\\] \\_a\\_\n",
            Some((Note, &["2 * 3 [x] _a_"])),
        ),
        // CommonMark only escapes ASCII punctuation: `\s` stays, `\\` is one `\`.
        (
            "> [!WARNING]\n> Breaks on \\\\server\\share.\n",
            Some((Warning, &["Breaks on \\server\\share."])),
        ),
    ]);
}

/// Text in backticks is literal: only the backticks go.
#[test]
fn code_spans_are_literal() {
    check(&[
        (
            "> [!NOTE]\n> at `\\\\server\\share` now\n",
            Some((Note, &["at \\\\server\\share now"])),
        ),
        ("> [!NOTE]\n> `**kwargs`\n", Some((Note, &["**kwargs"]))),
        (
            "> [!NOTE]\n> `[[plan|x]]` `[a](b)` `a_b_`\n",
            Some((Note, &["[[plan|x]] [a](b) a_b_"])),
        ),
        ("> [!NOTE]\n> ``a ` b``\n", Some((Note, &["a ` b"]))),
    ]);
}

/// Lists and code inside a Callout.
#[test]
fn bullets_get_glyphs_and_code_lines_are_kept() {
    check(&[
        (
            "> [!NOTE]\n> - [x] done\n> - [ ] todo\n> * [X] star\n> + plain\n> - bullet\n",
            Some((Note, &["☑ done", "☐ todo", "☑ star", "• plain", "• bullet"])),
        ),
        (
            "> [!TIP]\n> run:\n> ```sh\n> cargo **test**\n>\n>   ls\n> ```\n> done\n",
            Some((Tip, &["run:", "cargo **test**", "  ls", "done"])),
        ),
    ]);
}

#[test]
fn shown_cuts_to_the_first_lines_and_says_when_more_follow() {
    let callout = first("> [!NOTE]\n> a\n> b\n> c\n").unwrap();
    assert_eq!(callout.shown(2), (&callout.lines[..2], true));
    assert_eq!(callout.shown(3), (&callout.lines[..], false));
    assert_eq!(callout.shown(5), (&callout.lines[..], false));
}

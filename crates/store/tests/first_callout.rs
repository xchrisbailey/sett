use sett_store::{CalloutType, Date, Space, Store, Task, TaskCallout};

fn task_with_body(body: &str) -> Task {
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
    task
}

fn first(body: &str) -> Option<TaskCallout> {
    task_with_body(body).first_callout()
}

fn callout(kind: CalloutType, lines: &[&str]) -> Option<TaskCallout> {
    Some(TaskCallout {
        kind,
        lines: lines.iter().map(|l| l.to_string()).collect(),
    })
}

#[test]
fn no_body_and_no_callout_give_none() {
    assert_eq!(first(""), None);
    assert_eq!(first("# Title\n\nJust text.\n"), None);
    assert_eq!(first("> a plain quote\n"), None);
}

#[test]
fn finds_a_callout_after_other_blocks() {
    let body = "Intro\n\n- a\n- b\n\n> plain\n\n> [!TIP]\n> Use it.\n";
    assert_eq!(first(body), callout(CalloutType::Tip, &["Use it."]));
}

#[test]
fn every_type_in_any_case() {
    for (marker, kind) in [
        ("NOTE", CalloutType::Note),
        ("tip", CalloutType::Tip),
        ("Important", CalloutType::Important),
        ("wArNiNg", CalloutType::Warning),
        ("CAUTION", CalloutType::Caution),
    ] {
        assert_eq!(
            first(&format!("> [!{marker}]\n> x\n")),
            callout(kind, &["x"])
        );
    }
    assert_eq!(first("> [!FOO]\n> x\n"), None);
}

#[test]
fn three_space_indent_still_counts() {
    assert_eq!(
        first("   > [!NOTE]\n   > x\n"),
        callout(CalloutType::Note, &["x"])
    );
}

#[test]
fn a_callout_in_a_list_item_is_ignored() {
    assert_eq!(first("- item\n  > [!NOTE]\n  > x\n"), None);
    assert_eq!(first("1. item\n   > [!NOTE]\n   > x\n"), None);
    // ... and the scan goes on to a top-level one after the list.
    assert_eq!(
        first("- item\n  > [!NOTE]\n  > in\n\n> [!TIP]\n> out\n"),
        callout(CalloutType::Tip, &["out"])
    );
}

#[test]
fn a_callout_right_after_a_list_item_counts() {
    assert_eq!(
        first("- item\n> [!NOTE]\n> x\n"),
        callout(CalloutType::Note, &["x"])
    );
}

#[test]
fn a_callout_in_a_quote_is_ignored() {
    assert_eq!(first("> > [!NOTE]\n> > x\n"), None);
    assert_eq!(first("> text\n> [!NOTE]\n> x\n"), None);
}

#[test]
fn fenced_code_is_skipped() {
    assert_eq!(first("```\n> [!NOTE]\n> x\n```\n"), None);
    assert_eq!(first("~~~\n> [!NOTE]\n> x\n~~~\n"), None);
    // An unterminated fence swallows the rest.
    assert_eq!(first("```\n> [!NOTE]\n> x\n"), None);
    assert_eq!(
        first("```\n> [!NOTE]\n> x\n```\n> [!TIP]\n> y\n"),
        callout(CalloutType::Tip, &["y"])
    );
}

#[test]
fn indented_code_is_skipped() {
    assert_eq!(first("para\n\n    > [!NOTE]\n    > x\n"), None);
}

#[test]
fn text_after_the_marker_or_an_escape_makes_a_plain_quote() {
    assert_eq!(first("> [!NOTE] inline\n> x\n"), None);
    assert_eq!(first("> \\[!NOTE]\n> x\n"), None);
}

#[test]
fn crlf_works() {
    assert_eq!(
        first("> [!WARNING]\r\n> careful\r\n> now\r\n"),
        callout(CalloutType::Warning, &["careful", "now"])
    );
}

#[test]
fn an_empty_callout_is_skipped() {
    assert_eq!(first("> [!NOTE]\n"), None);
    assert_eq!(first("> [!NOTE]\n>\n>  \n"), None);
    assert_eq!(
        first("> [!NOTE]\n\ntext\n\n> [!TIP]\n> second\n"),
        callout(CalloutType::Tip, &["second"])
    );
    assert_eq!(
        first("> [!NOTE]\n>\n\n> [!CAUTION]\n> second\n"),
        callout(CalloutType::Caution, &["second"])
    );
}

#[test]
fn lazy_continuation_lines_belong_to_the_callout() {
    assert_eq!(
        first("> [!NOTE]\nlazy\n\nafter\n"),
        callout(CalloutType::Note, &["lazy"])
    );
    assert_eq!(
        first("> [!NOTE]\n> one\ntwo\n\nafter\n"),
        callout(CalloutType::Note, &["one", "two"])
    );
}

#[test]
fn the_callout_ends_at_a_blank_line() {
    assert_eq!(
        first("> [!NOTE]\n> a\n\n> b\n"),
        callout(CalloutType::Note, &["a"])
    );
}

#[test]
fn the_first_of_two_callouts_wins() {
    assert_eq!(
        first("> [!NOTE]\n> one\n\n> [!TIP]\n> two\n"),
        callout(CalloutType::Note, &["one"])
    );
}

#[test]
fn blank_source_lines_are_left_out() {
    assert_eq!(
        first("> [!NOTE]\n> a\n>\n> b\n"),
        callout(CalloutType::Note, &["a", "b"])
    );
}

#[test]
fn emphasis_and_code_markers_are_stripped() {
    assert_eq!(
        first("> [!NOTE]\n> **bold** __also__ *it* _em_ `code` snake_case\n"),
        callout(CalloutType::Note, &["bold also it em code snake_case"])
    );
}

#[test]
fn links_become_their_text() {
    assert_eq!(
        first("> [!NOTE]\n> see [the **docs**](https://x.y/z) now\n"),
        callout(CalloutType::Note, &["see the docs now"])
    );
}

#[test]
fn wiki_links_become_their_label_or_slug() {
    assert_eq!(
        first("> [!NOTE]\n> [[plan|the plan]] [[plan]] [[plan#Heading]]\n"),
        callout(CalloutType::Note, &["the plan plan plan"])
    );
}

#[test]
fn bullets_and_checkboxes_get_glyphs() {
    assert_eq!(
        first("> [!NOTE]\n> - [x] done\n> - [ ] todo\n> * [X] star\n> + plain\n> - bullet\n"),
        callout(
            CalloutType::Note,
            &["☑ done", "☐ todo", "☑ star", "• plain", "• bullet"]
        )
    );
}

#[test]
fn backslash_escapes_are_removed() {
    assert_eq!(
        first("> [!NOTE]\n> 2 \\* 3 \\[x\\] \\_a\\_\n"),
        callout(CalloutType::Note, &["2 * 3 [x] _a_"])
    );
}

#[test]
fn fence_lines_are_dropped_and_code_lines_kept() {
    assert_eq!(
        first("> [!TIP]\n> run:\n> ```sh\n> cargo **test**\n>\n>   ls\n> ```\n> done\n"),
        callout(
            CalloutType::Tip,
            &["run:", "cargo **test**", "  ls", "done"]
        )
    );
}

#[test]
fn a_quote_with_only_a_fence_is_empty() {
    assert_eq!(first("> [!NOTE]\n> ```\n> ```\n"), None);
}

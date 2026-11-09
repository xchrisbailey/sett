//! Loading Markdown into the headless model and saving it again must not
//! rewrite what the author (or an agent) wrote.

use gpui_kit::base::text::MarkdownExtensions;
use markdown_editor::BlockEditorModel;

fn save(source: &str) -> String {
    BlockEditorModel::from_markdown(source, MarkdownExtensions::default())
        .expect("fixture parses")
        .source()
}

#[track_caller]
fn assert_round_trips(source: &str) {
    assert_eq!(save(source), source);
}

#[test]
fn trailing_line_endings_are_kept_exactly() {
    for source in [
        "no newline",
        "one newline\n",
        "blank line after\n\n",
        "two blank lines\n\n\n",
        "# Heading\n",
        "- item\n",
        "```\ncode\n```\n",
        "crlf\r\n",
    ] {
        assert_round_trips(source);
    }
}

#[test]
fn wiki_links_stay_verbatim() {
    assert_round_trips(
        "See [[Release process]], [[Meeting notes|last week's notes]] and [[Inbox#Today]].\n",
    );
    assert_round_trips("- [ ] Review [[Spike: block editor]] results\n");
}

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
    assert_round_trips("Linked [[snake_case note]] and [[R&D]].\n");
    // Inside a table an unescaped `|` would split the cell.
    assert_round_trips("| Note |\n| --- |\n| [[target\\|label]] |\n");
}

#[test]
fn soft_line_breaks_are_kept() {
    for source in [
        "Paragraph right after headings.\nSoft-wrapped second line.\n",
        "> We should not save files the user only opened. Saving on view\n> rewrites Markdown that agents wrote.\n>\n> — Priya\n",
        "- An item that wraps\n  onto a second line.\n- Next item\n",
        "1. Ordered item\n   continues here.\n",
        "**09:12** started the sync\n**09:40** *found the bug\nin the lock*\n",
        "A [link that\nwraps](notes.md) mid-text.\n",
    ] {
        assert_round_trips(source);
    }
}

#[test]
fn literal_brackets_that_are_not_wiki_links_stay_escaped() {
    for source in ["\\[\\[not a link\\]\\](notes.md)\n", "\\[\\[a\\*b\\]\\]\n"] {
        assert_round_trips(source);
    }
}

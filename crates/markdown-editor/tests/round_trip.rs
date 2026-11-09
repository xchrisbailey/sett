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

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"))
}

/// Fixtures whose bytes survive a load and save unchanged.
const IDENTICAL: &[&str] = &["01-checklist.md", "07-long-note.md"];

/// Fixtures that use syntax the serializer normalises (see FORK.md); they
/// only have to settle after one save.
const NORMALISED: &[&str] = &[
    "02-table.md",
    "03-code.md",
    "04-quote-and-marks.md",
    "05-headings.md",
    "06-agent-daily.md",
];

#[test]
fn fixtures_round_trip_byte_for_byte() {
    for name in IDENTICAL {
        let source = fixture(name);
        assert_eq!(save(&source), source, "{name}");
    }
}

#[test]
fn every_fixture_settles_after_one_save() {
    for name in IDENTICAL.iter().chain(NORMALISED) {
        let once = save(&fixture(name));
        assert_eq!(save(&once), once, "{name}");
    }
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
fn list_markers_are_kept() {
    for source in [
        "* star\n* bullets\n",
        "+ plus\n+ bullets\n",
        "1) paren\n2) ordered\n",
        "3. dot\n4. ordered\n",
        "- outer\n  * inner star\n",
        "> * quoted star\n",
        // Different markers make separate lists; they must not merge.
        "- dash\n\n* star\n",
    ] {
        assert_round_trips(source);
    }
}

#[test]
fn callout_markers_stay_verbatim() {
    for source in [
        "> [!NOTE]\n> This is a GitHub-style callout.\n",
        "> [!warning]- Folded\n> body\n",
    ] {
        assert_round_trips(source);
    }
}

#[test]
fn literal_brackets_that_are_not_wiki_links_stay_escaped() {
    for source in [
        "Mid-text \\[!NOTE\\] is not a callout.\n",
        "\\[\\[not a link\\]\\](notes.md)\n",
        "\\[\\[a\\*b\\]\\]\n",
    ] {
        assert_round_trips(source);
    }
}

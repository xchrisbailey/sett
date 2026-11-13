//! The store's `Task::first_callout` scan must agree with the editor's
//! Markdown import about which Callout a body holds. The scan lives in the
//! store (which can't link the editor), so this test keeps the two in step.

use gpui_kit::base::text::MarkdownExtensions;
use markdown_editor::{BlockEditorModel, BlockKind, BlockRef, CalloutKind};
use sett_store::{CalloutKind as TaskCalloutKind, Date, Space, Store, Task};

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

#[test]
fn every_callout_kind_maps_to_the_editors_kind_by_name_and_back() {
    for task_kind in TaskCalloutKind::ALL {
        let kind = CalloutKind::from_marker(task_kind.marker()).expect("editor knows the kind");
        assert_eq!(kind.marker(), task_kind.marker());
        assert_eq!(TaskCalloutKind::from_marker(kind.marker()), Some(task_kind));
    }
    assert_eq!(TaskCalloutKind::ALL.len(), CalloutKind::ALL.len());
    for kind in CalloutKind::ALL {
        assert!(TaskCalloutKind::from_marker(kind.marker()).is_some());
    }
}

fn has_text(block: BlockRef<'_>) -> bool {
    block.text().is_some_and(|t| !t.trim().is_empty()) || block.children().any(has_text)
}

/// The type of the first non-empty top-level Callout the editor draws.
fn editor_first_callout(body: &str) -> Option<String> {
    let model = BlockEditorModel::from_markdown(body, MarkdownExtensions::default()).ok()?;
    model
        .document()
        .root()
        .children()
        .find_map(|block| match block.kind() {
            BlockKind::Blockquote {
                callout: Some(callout),
            } if has_text(block) => Some(callout.marker().to_string()),
            _ => None,
        })
}

const SAMPLES: &[&str] = &[
    "",
    "# Title\n",
    "> [!NOTE]\n> one\n",
    "para\n\n- a\n\n> plain\n\n> [!tip]\n> two\n",
    "- x\n  > [!NOTE]\n  > in list\n",
    "> > [!NOTE]\n> > nested\n",
    "```\n> [!NOTE]\n> fenced\n```\n",
    "~~~\n> [!NOTE]\n> tilde\n~~~\n",
    "para\n\n    > [!NOTE]\n    > indented\n",
    "```\n> [!NOTE]\n> unterminated\n",
    "   > [!WARNING]\n   > three spaces\n",
    "> [!NOTE] inline text\n> x\n",
    "> \\[!NOTE]\n> escaped\n",
    "> [!CAUTION]\r\n> crlf\r\n",
    "> [!NOTE]\n",
    "> [!NOTE]\n>\n\n> [!IMPORTANT]\n> second\n",
    "> [!NOTE]\n\ntext\n\n> [!TIP]\n> second\n",
    "> plain\n\n> [!CAUTION]\n> c\n",
    "> [!NOTE]\n> a\n\n> [!TIP]\n> b\n",
    "- item\n> [!NOTE]\n> after list\n",
    "> [!NOTE]\nlazy\n",
    "- item\nlazy\n  > [!NOTE]\n  > x\n",
    "1. item\nlazy\n   > [!NOTE]\n   > x\n\n> [!TIP]\n> out\n",
    "> [!NOTE]\n> para\n>\n> - a\n> - b\n>\n> ```\n> code\n> ```\n",
    "---\ntitle: x\n---\n\n> [!TIP]\n> after rule\n",
    "> [!FOO]\n> unknown\n",
    include_str!("../../markdown-editor/tests/fixtures/01-checklist.md"),
    include_str!("../../markdown-editor/tests/fixtures/04-quote-and-marks.md"),
    include_str!("../../markdown-editor/tests/fixtures/07-long-note.md"),
];

#[test]
fn first_callout_picks_the_same_type_as_the_editors_import() {
    for body in SAMPLES {
        let scan = task_with_body(body)
            .first_callout()
            .map(|c| c.kind.marker().to_string());
        assert_eq!(scan, editor_first_callout(body), "body: {body:?}");
    }
}

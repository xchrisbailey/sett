---
status: proposed (pending macOS spike)
---

# Block editing uses gpui-kit-markdown-editor

Items are edited in a Notion-style block view from `gpui-kit-markdown-editor`, with gpui-kit's `Editor` as a raw Markdown mode that one global toggle switches to. We chose this over writing our own rich-text engine (months of work, and the GPUI ecosystem already has several) and over a homemade click-to-edit block view (less like Notion, and no selection across blocks). The crate is early and so far tested only on Linux, so we adopt it only after a throwaway spike on macOS shows that typing, input methods, undo and round-tripping agent-written Markdown all work. macOS fixes go upstream.

## Consequences

- sett writes an Item's file only after the user has actually edited it. Opening or viewing an Item never saves it, because the editor normalises Markdown on save.
- `[[wiki-links]]` may show as plain text in the block view at first.

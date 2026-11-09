# Meeting notes — 2026-11-03

Attendees: Sam, Priya, *the agent*

> We should not save files the user only opened. Saving on view
> rewrites Markdown that agents wrote, and that shows up as noise in git.
>
> — Priya

Decisions:

- Keep **bold**, *italic*, ***both***, ~~strike~~ and `inline code` as written.
- Underscore variants: __bold__ and _italic_ should survive too.
- Links: [the ADR](docs/adr/0003-block-editor.md) and <https://example.com>.
- Wiki-links: [[ADR 0003]], [[Meeting notes|last week's notes]], [[Inbox#Today]].

Some text with a hard line break at the end of this line  
and a backslash break here\
and then normal text.

---

Escapes: 5 \* 3 = 15, a literal \[bracket\], and an_underscore_word.

Emoji and Unicode: café, naïve, 日本語, 🚀.

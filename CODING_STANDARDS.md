# Coding standards

Read during review. These are judgement rules. Formatting, lints, tests and conflict markers are enforced by `.github/workflows/ci.yml` and `.githooks/pre-commit`, so skip them.

## Sources

Apply each of these in full:

- `GLOSSARY.md`: the domain vocabulary.
- `docs/adr/`: decisions that have to hold.
- `.agents/skills/gpui-kit/references/coding-guides.md`: GPUI and gpui-kit architecture. Most relevant: "Rules for coding agents", "Common failure modes" and "Implementation checklist".
- `.agents/skills/gpui-kit-design-guides/`: any change with a visible surface.
- The user's global rules, `~/.config/opencode/AGENTS.md`:
  - comments explain why;
  - no casts to get around type errors;
  - dependencies only by agreement;
  - few tests, focused on integration, validation, state and error handling.

## Project rules

- **Glossary names.** Name types, modules and public functions with `GLOSSARY.md` terms, never their _Avoid_ entries. v1 shipped `Workspace` and `Backlink` and had to rename them to `AppModel` and `InboundLink`.
- **Files are sacred (ADR 0001, 0003).** sett writes an Item only after a real user edit, and never writes a file whose frontmatter didn't parse. Any new path that writes files needs a test asserting the bytes stay unchanged when nothing was edited or the frontmatter is broken.
- **Spaces stay isolated (ADR 0002).** Nothing indexes, links to or searches across both Spaces.
- **Everything clickable is reachable from the keyboard.** It's focusable and opens on Enter. Board Cards and Note rows set the pattern.
- **Visible changes come with a screenshot.** Capture the changed screen with `scripts/screenshot` and look at it before calling the work done.

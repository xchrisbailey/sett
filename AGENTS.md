## Agent skills

### Issue tracker

Issues live in GitHub Issues on xchrisbailey/sett (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.

## Working here

- **gpui-kit APIs:** check `docs/agents/gpui-kit-notes.md` first. It was verified against the 0.9.1 source, and the published docs lag behind it. It also covers testing gotchas.
- **UI changes:** `scripts/screenshot OUT.png` captures the real window against fixture data. Read the PNG to check your layout.
- **Parallel worktrees:** `docs/agents/worktrees.md`.
- **Reviewing:** `CODING_STANDARDS.md`.
- **Checks:** CI (`.github/workflows/ci.yml`) runs fmt, clippy and tests. Enable the pre-commit hook once per clone with `git config core.hooksPath .githooks`.

# Release checklist for v0.4

Tracking the work needed before we cut **v0.4**. See [[Release process]] for the general flow.

## Before the freeze

- [x] Merge the parser rewrite (#212)
- [ ] Update the changelog
  - [x] Collect merged PRs since `v0.3.2`
  - [ ] Group them by area
    - [ ] Editor
    - [ ] Sync
  - [ ] Write the *highlights* paragraph
- [ ] Bump versions in `Cargo.toml`
- [ ] Run the full test matrix on macOS, Linux and Windows

## After the freeze

1. Tag the release candidate.
2. Announce in [[Team channel]] and ask for testing.
3. Wait at least **48 hours** for reports.
   - If a blocker shows up, fix it on `release/0.4` and go back to step 1.
   - Otherwise continue.
4. Publish.

## Notes

* Remember that `cargo publish` needs the crates in dependency order.
* The docs site rebuilds on its own; no action needed.

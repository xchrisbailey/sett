# Fork of gpui-kit-markdown-editor

- Upstream: https://github.com/wabzqem/gpui-kit-markdown-editor
- Base version: 0.2.0 (crates.io)
- License: Apache-2.0 (see `LICENSE-APACHE`)
- Package name `gpui-kit-markdown-editor` and lib name `markdown_editor` are unchanged.

## Patches we carry

Each line is a candidate for upstreaming.

- `src/direct.rs`: set `task_marker: None` on `BlockNode::ListItem`, a field gpui-kit 0.9.1 added. Without it the crate doesn't compile against gpui-kit ^0.9.1.
- `Cargo.toml`, `src/tests/mod.rs`, `src/component.rs`, `src/state.rs`, `src/projection.rs`: GPUI view tests are gated behind a `view-tests` feature, and the `gpui-kit/test-support` dev-dependency is dropped. In our workspace it pulls in `gpui-fast`, whose pins don't resolve. Not an upstream fix, only a workaround for our build.
- `Cargo.toml`: depends on the workspace `gpui-kit`. The upstream `Cargo.lock` is removed.

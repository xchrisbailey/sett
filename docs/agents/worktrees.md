# Parallel worktrees

How to run several agents on separate branches at the same time.

- **One worktree per branch**, outside the repo: `git worktree add -b <branch> ../sett-wt/<name> <base>`.
- **Give each worktree its own `CARGO_TARGET_DIR`**, for example `../sett-wt/.target-<name>`. With one shared target dir across worktrees, Cargo once linked another worktree's `sett-store` (same package name and version, different path), and the tests ran against the wrong code.
- **Budget disk space.** Each target dir grows to 6–19 GB, because the GPUI dependency tree is large.
- **Implementer branches stay local.** Merge them into the integration branch locally, and push only the integration branch. Deleting remote branches is blocked for agents.
- **Clean up** once the branch is merged: `git worktree remove ../sett-wt/<name>`, `git branch -d <branch>`, then delete its target dir.
- **Optional:** set `RUSTC_WRAPPER=sccache` to share compiled dependencies across target dirs safely. It cuts both build time and disk use. Install it first with `brew install sccache`.

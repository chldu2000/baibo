# Baibo

Baibo is a local-first desktop workspace for running and coordinating multiple
terminal-based coding agents.

The MVP targets:

- Tauri 2 desktop shell
- Svelte 5 + SvelteKit SPA UI
- macOS as the first supported platform
- Codex CLI and the Pi coding agent (`pi`) as the first agent providers
- Multiple workspaces and multiple isolated agent sessions per workspace
- PTY-backed terminals, Git worktree isolation, file-change review, and durable
  session metadata
- Cross-agent context references through a provider-neutral `baibo://` protocol
  and a local context service

Product goals and system requirements are documented in
[`docs/system-spec.md`](docs/system-spec.md).

The implementation and acceptance sequence is documented in
[`docs/mvp-checkpoints.md`](docs/mvp-checkpoints.md).

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

## Development

Prerequisites:

- Node.js 22+
- Rust 1.77.2+
- macOS Tauri system dependencies

Commands:

```bash
npm install
npm run tauri dev
```

Validation:

```bash
npm run check
npm run lint
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

## Structure

```text
src/
  lib/domain/       Frontend domain types
  lib/ipc/          Typed Tauri clients
  routes/           SvelteKit SPA routes
src-tauri/src/
  adapters/         Agent and infrastructure adapters
  commands/         Tauri command boundary
  domain/           Rust domain types
  persistence/      Durable storage
  services/         Application services
```

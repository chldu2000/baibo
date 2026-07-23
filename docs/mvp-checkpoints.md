# Baibo MVP Checkpoints

## 1. Checkpoint Rules

Each checkpoint must:

- Produce a user-visible or architecture-validating increment.
- Include automated tests where the repository has test coverage.
- Include a manual acceptance script for PTY and provider behavior.
- Leave the application runnable.
- Avoid depending on features assigned to later checkpoints.

A checkpoint is complete only when all exit criteria pass.

## CP0 — Project Foundation

### Goal

Create a buildable Tauri 2 and Svelte 5 application with clear frontend/backend
boundaries.

### Scope

- Initialize Tauri 2.
- Initialize SvelteKit with TypeScript and static adapter.
- Configure SPA mode.
- Add formatting, linting, type checking, Rust formatting, and Rust linting.
- Add a minimal application shell.
- Define directories for Rust domain, services, adapters, persistence, and Tauri
  commands.
- Define directories for Svelte routes, components, workspace state, and IPC
  clients.

### Exit Criteria

- Development application launches.
- Production build succeeds.
- Svelte type check succeeds.
- Rust tests and lints succeed.
- One typed Tauri command can be invoked from Svelte.
- No arbitrary shell permission is exposed to the frontend.

## CP1 — Workspace Registry

### Goal

Manage multiple local workspaces durably.

### Scope

- SQLite setup and migrations.
- Workspace domain model.
- Register, list, open, rename, and remove registration.
- Canonical path validation and duplicate detection.
- Git repository detection.
- Workspace navigation UI.

### Exit Criteria

- Two repositories can be registered and switched.
- Duplicate canonical paths are rejected.
- Removing a workspace does not delete repository files.
- Restart preserves registrations and last-opened workspace.
- Workspace-scoped queries cannot return another workspace's records.

## CP2 — PTY Terminal Runtime

### Goal

Run a real interactive terminal process inside Baibo.

### Scope

- Rust PTY manager.
- macOS PTY implementation.
- Process spawn, input, output, resize, exit, stop.
- xterm.js terminal component.
- Bounded terminal scrollback and persisted log chunks.
- Separate PTY and structured event channels.

### Exit Criteria

- Interactive shell runs inside Baibo.
- ANSI colors, resize, UTF-8, copy, paste, Ctrl+C, and exit work.
- Closing a terminal component releases subscriptions without killing unrelated
  sessions.
- A process crash does not crash Baibo.
- Terminal output is never rendered as HTML.

## CP3 — Codex and Pi Agent Adapters

### Goal

Launch the first two supported coding agents.

### Scope

- Provider detection and version display.
- Provider capability records.
- Codex launch adapter.
- Pi launch adapter.
- Pi interactive PTY launch mode.
- Pi RPC capability declaration and JSONL protocol spike.
- Pi project-trust state detection and user-facing guidance.
- Session create, stop, restart, and exit state.
- Existing provider authentication reuse.
- User-facing unavailable-provider diagnostics.

### Exit Criteria

- Codex and Pi can each be launched in a selected workspace.
- Provider executable and version are visible.
- Missing executable produces an actionable error.
- Launch arguments are arrays and are not shell-interpolated.
- Credentials are not copied to Svelte state or persisted launch configuration.
- Baibo does not silently approve a Pi project or overwrite Pi user settings.
- A Pi RPC smoke test can exchange valid line-delimited JSON without coupling
  the MVP terminal session to RPC mode.
- Pi TUI text is not parsed as authoritative messages, tools, or approvals.

## CP4 — Durable Sessions and Recovery

### Goal

Make Baibo state survive application restart.

### Scope

- AgentSession persistence.
- Lifecycle event persistence.
- Terminal log indexing.
- Startup reconciliation.
- Interrupted-process classification.
- Session list and session detail UI.

### Exit Criteria

- Restart restores session metadata and logs.
- Dead child processes are not displayed as running.
- Baibo does not silently relaunch agents.
- Lifecycle events remain ordered and duplicate-safe.
- One failed session does not corrupt another session.

## CP5 — Git Worktrees and Change Review

### Goal

Isolate writable agents and make their changes reviewable.

### Scope

- Worktree create and registration.
- Session branch naming.
- Base commit and head tracking.
- Changed-file list.
- Unified diff view.
- Dirty-state and external-change detection.
- Explicit cleanup confirmation.

### Exit Criteria

- Two agents can modify the same repository in separate worktrees.
- Changes from one worktree do not appear in the other's diff.
- The UI shows base commit, branch, changed files, and diff.
- Restart reconciles worktree records with Git.
- Cleanup refuses to remove an active or referenced worktree.

## CP6 — Tasks, Artifacts, and Context Index

### Goal

Create durable objects that can be referenced across agents.

### Scope

- Task CRUD and state transitions.
- Artifact registration and hashing.
- Session result summary.
- ChangeSet snapshots.
- ContextReference records.
- Search index for sessions, tasks, artifacts, files, and changesets.

### Exit Criteria

- A task can be assigned to an agent session.
- A completed session can attach a summary and artifacts.
- Artifacts record provenance and hash.
- Search results are workspace-scoped.
- Deleted or inaccessible objects produce tombstoned or missing references
  rather than resolving to another object.

## CP7 — Big `@` Picker and URI Protocol

### Goal

Insert stable, provider-neutral references from the UI.

### Scope

- Big `@` composer.
- Picker for session, task, artifact, file, and changeset.
- `baibo://` URI parser and serializer.
- Human-readable mention chips.
- Revision selection.
- Paste and round-trip support.

### Exit Criteria

- Every required entity type can be selected.
- Mention labels can change without changing entity identity.
- URI parse/serialize round trips are deterministic.
- Malformed and cross-workspace URIs are rejected.
- The picker never embeds a full transcript or credential.

## CP8 — Context Service and Agent Injection

### Goal

Allow Codex and Pi to resolve Big `@` references on demand.

### Scope

- Transport-neutral local context service.
- Local MCP adapter.
- `baibo-context` CLI.
- Workspace/session-scoped authentication.
- Summary, detail, and paginated conversation views.
- Context limits and truncation metadata.
- `agent-context` skill.
- `AGENTS.md` routing guidance.
- Codex MCP/runtime injection.
- Pi shared-skill and CLI injection.
- Generated Pi extension spike for a native `baibo_context` tool.
- Pi project-trust handling.
- Initial prompt fallback.

### Exit Criteria

- Codex resolves a Baibo session reference.
- Pi resolves the same reference through the shared skill and CLI.
- Both begin with bounded summary output.
- Detailed content requires an explicit follow-up tool call.
- Unauthorized and cross-workspace resolution fails closed.
- Referenced text is marked as untrusted content.
- Existing user agent configuration is not overwritten.
- The Pi extension path and CLI path enforce the same authorization and limits
  as MCP.

## CP9 — End-to-End Handoff

### Goal

Complete the defining Baibo workflow.

### Scope

- Start Codex and Pi in isolated worktrees.
- Let one session produce changes, summary, test result, and artifact.
- Reference that session from the other agent.
- Resolve context through the provider-appropriate transport.
- Continue implementation in the receiving session.
- Review both change sets.

### Exit Criteria

- The workflow succeeds without manually copying a transcript.
- The receiving agent can identify source session, revision, changed files,
  tests, and artifacts.
- Missing detail is fetched selectively.
- The receiving agent cannot mutate source-session state through a read-only
  context reference.
- Evidence and provenance remain visible in the UI.

## CP10 — Security and Reliability Hardening

### Goal

Make the MVP safe enough for local daily use.

### Scope

- Path traversal and symlink tests.
- Workspace authorization tests.
- Secret redaction.
- Context prompt-injection boundaries.
- PTY, RPC, MCP, and context CLI backpressure.
- Database backup and migration failure handling.
- Crash recovery.
- Destructive-operation confirmation.
- Diagnostic export.

### Exit Criteria

- Cross-workspace access tests fail closed.
- Symlink escape tests fail closed.
- Known credential patterns are absent from diagnostics.
- Large PTY output does not freeze the UI.
- Large context results truncate and paginate.
- Database migration failure produces recovery guidance.
- Destructive actions cannot be triggered from terminal output or referenced
  content.

## CP11 — macOS Release Candidate

### Goal

Package and validate the first distributable Baibo build.

### Scope

- macOS bundle metadata and icons.
- Code signing and notarization plan.
- Clean-machine installation test.
- Provider detection on a clean user account.
- Upgrade and data-migration test.
- User documentation and known limitations.

### Exit Criteria

- Packaged application installs and launches on a clean test machine.
- Existing Codex and Pi installations are detected.
- The CP9 end-to-end workflow passes in the packaged build.
- Upgrade preserves SQLite state and worktree metadata.
- Known limitations explicitly state macOS-only MVP support and provider-specific
  session behavior.

## 2. MVP Critical Path

```text
CP0
 -> CP1
 -> CP2
 -> CP3
 -> CP4
 -> CP5
 -> CP6
 -> CP7
 -> CP8
 -> CP9
 -> CP10
 -> CP11
```

CP6 data modeling can begin after CP1. CP5 and early CP6 implementation may
proceed in parallel once CP4 session identity is stable.

## 3. Deferred Post-MVP Checkpoints

- Linux PTY and packaging
- Windows ConPTY and packaging
- Structured Codex app-server adapter
- Full Pi RPC-mode conversation UI
- Cursor Agent and other provider adapters
- ACP provider support
- Native provider session resume and fork UI
- Unified approval center
- Automated worktree merge and conflict assistance
- Remote execution hosts
- Multi-user rooms and cloud synchronization
- Usage and cost accounting
- Extension marketplace

# Baibo System Goals and MVP Specification

## 1. Document Status

- Product: Baibo
- Status: Initial MVP specification
- Target platform: macOS first
- Desktop stack: Tauri 2
- Frontend stack: Svelte 5, SvelteKit SPA, TypeScript
- Initial agent providers: Codex CLI and the Pi coding agent (`pi`)
- Storage model: Local-first

## 2. Product Definition

Baibo is a desktop client for managing multiple development workspaces. Each
workspace can contain multiple terminal-based coding agent sessions. Baibo
provides a common UI for terminals, session state, context references, tasks,
artifacts, Git worktrees, and file changes without attempting to replace the
underlying agents.

Baibo treats the terminal as a compatibility and interaction surface, not as
the sole source of system state. Durable workspace state and cross-agent context
are owned by Baibo.

## 3. System Goals

### 3.1 Primary Goals

1. Let a user register, open, and switch between multiple local repositories.
2. Let a user run multiple coding agents in one workspace without manually
   managing terminal windows.
3. Isolate write-capable sessions with Git worktrees to reduce accidental
   overwrites.
4. Make each agent's terminal, lifecycle, file changes, and artifacts visible
   from one interface.
5. Allow one agent to reference another agent's work through stable,
   provider-neutral context references.
6. Persist enough state to restore the Baibo workspace after an application
   restart.
7. Preserve the user's existing agent authentication and subscription setup.
8. Enforce workspace and session boundaries when resolving context or accessing
   files.

### 3.2 Success Scenario

The MVP is successful when a user can:

1. Add two local repositories as Baibo workspaces.
2. Open one workspace and start a Codex session and a Pi session.
3. Run both sessions in separate worktrees.
4. Interact with both agents through embedded terminals.
5. Select a previous agent session, task, artifact, or file change from a Big
   `@` picker.
6. Send the resulting `baibo://` reference to another agent.
7. Have that agent retrieve a bounded summary or selected details through the
   Baibo context service.
8. Review each session's changed files and diff.
9. Quit and reopen Baibo without losing workspace registration, session
   metadata, terminal logs, tasks, or context references.

## 4. Explicit Non-Goals for MVP

- Multi-user collaboration or cloud synchronization
- Mobile clients
- Remote execution hosts
- Windows and Linux production support
- Reproducing private model context, hidden reasoning, or provider caches
- A provider-independent replacement for native session resume or fork
- Automated merging of agent worktrees
- A plugin marketplace
- Unified billing or model usage accounting
- Full IDE features such as language servers, debugging, or code completion
- Automatic approval of arbitrary commands
- Pixel-perfect reconstruction of provider-native conversation UIs

## 5. Product Principles

### 5.1 Local-First

Workspace files, agent logs, context indexes, and credentials remain on the
local machine unless the underlying agent provider transmits them as part of
its own operation.

### 5.2 Provider-Neutral Core

Baibo's domain model must not contain Codex- or Pi-specific state names.
Provider-specific concepts are translated by adapters.

### 5.3 References Instead of Transcript Copies

Cross-agent context uses stable references and bounded, on-demand retrieval.
Baibo must not automatically insert complete conversation histories into every
prompt.

### 5.4 Explicit Isolation

Writable parallel sessions use separate worktrees by default. Sharing one
working directory requires an explicit user choice and a visible warning.

### 5.5 Evidence Over Claims

Agent results should link to changed files, commits, commands, test results, and
artifacts whenever those records are available.

### 5.6 Least Privilege

The Svelte frontend cannot spawn arbitrary processes or read arbitrary files.
All privileged actions pass through typed Tauri commands and Rust-side policy
checks.

### 5.7 Terminal-Native Interface

The terminal is Baibo's primary interaction surface, not a decorative motif.
Application chrome uses a restrained TUI-like visual language and
keyboard-first interaction while preserving modern readability, accessibility,
and platform conventions. Normative frontend rules are defined in
[`frontend-ui-constraints.md`](frontend-ui-constraints.md).

## 6. User Roles

The MVP has one local user role:

- Owner: Can register workspaces, start agents, approve terminal actions,
  inspect context, and manage local state.

Multi-user membership and role-based access control are deferred.

## 7. Functional Requirements

### 7.1 Workspace Management

Baibo must:

- Register a local directory as a workspace.
- Verify that the directory exists and is readable.
- Detect whether the directory is a Git repository.
- Store a display name, canonical path, repository root, and last-opened time.
- List, open, rename, and remove workspace registrations.
- Never delete the underlying repository when removing a registration.
- Prevent duplicate registrations of the same canonical path.
- Keep all workspace-scoped queries isolated by `workspaceId`.

### 7.2 Agent Provider Detection

Baibo must:

- Detect installed Codex CLI (`codex`) and Pi (`pi`) executables.
- Record executable path and detected version.
- Report unavailable or unsupported providers without crashing.
- Allow the user to refresh provider detection.
- Use the user's existing provider login state.
- Avoid copying provider authentication tokens into the frontend or database.
- Treat Pi as an agent harness, not as a model provider; the models available
  through Pi depend on the user's Pi configuration and login state.

### 7.3 Agent Session Lifecycle

Baibo must:

- Create an agent session with workspace, provider, working directory,
  isolation mode, and launch options.
- Spawn the process through a PTY.
- Stream terminal output to the UI.
- Send keyboard input and paste events to the PTY.
- Resize the PTY when the terminal pane changes size.
- Track at least `starting`, `running`, `exited`, `failed`, and `stopped`.
- Record process exit code and termination reason.
- Support explicit stop and restart.
- Avoid inferring authoritative lifecycle state solely from rendered terminal
  text.
- Preserve session metadata after the child process exits.

### 7.4 Terminal UI

The terminal surface must:

- Render ANSI color and common TUI control sequences.
- Support UTF-8, selection, copy, paste, scrollback, resize, and focus.
- Support multiple terminal tabs or panes.
- Clearly display workspace, provider, session, worktree, and process state.
- Keep terminal byte streams separate from structured Baibo control events.
- Never render terminal output as HTML.

### 7.5 Session Persistence

Baibo must persist:

- Workspace registrations
- Agent session metadata
- Provider and executable metadata
- Launch configuration excluding secrets
- PTY start and exit events
- Bounded terminal logs
- Tasks and artifacts
- Context references and revisions
- Worktree metadata
- File-change snapshots

The MVP does not promise that an exited provider process can always be resumed
as the same native provider conversation. Native resume is provider-specific.

### 7.6 Git and Worktree Isolation

For Git repositories, Baibo must:

- Capture the base branch and base commit when creating a writable session.
- Create a dedicated worktree and branch for isolated sessions.
- Use a deterministic, collision-safe naming convention.
- Record worktree path, branch, base commit, and current head.
- Show changed files and unified diffs.
- Refresh changes on demand and after relevant session events.
- Detect dirty or externally modified worktrees.
- Prevent automatic worktree deletion while referenced by a session.
- Require explicit confirmation for discard, removal, or destructive cleanup.

For non-Git workspaces, Baibo may run read-only sessions or shared-directory
sessions with a visible limitation warning.

### 7.7 Tasks

Baibo must provide a minimal task record:

- ID
- Workspace ID
- Title
- Goal
- Status
- Assigned session
- Context references
- Result summary
- Artifacts
- Created and updated timestamps

Required task states:

- `todo`
- `in_progress`
- `blocked`
- `completed`
- `canceled`

Task automation and dependency graphs are deferred.

### 7.8 Artifacts

An artifact is a durable output such as:

- Markdown report
- API contract
- Test report
- Screenshot
- Generated asset
- Patch or diff

Baibo must record artifact identity, workspace, creator session, source turn or
task when known, file path or content-addressed storage key, media type, size,
hash, and creation time.

### 7.9 Big `@` Context Picker

The UI must provide a context picker capable of selecting:

- Agent sessions
- Session summaries
- Turns when structured turn data is available
- Tasks
- Artifacts
- Workspace files and folders
- Session change sets
- Test results

The selected item is serialized as a stable URI:

```text
baibo://workspace/<workspace-id>/session/<session-id>
baibo://workspace/<workspace-id>/task/<task-id>
baibo://workspace/<workspace-id>/artifact/<artifact-id>
baibo://workspace/<workspace-id>/changeset/<changeset-id>
baibo://workspace/<workspace-id>/file/<file-reference-id>
```

Requirements:

- The URI, not the visible label, is authoritative.
- Every reference is scoped to one workspace.
- References support immutable revisions where reproducibility matters.
- The picker must not insert raw credentials, absolute secret paths, or full
  transcripts.
- A missing or unauthorized reference must fail closed.

### 7.10 Context Resolution

Baibo must implement context resolution as a transport-neutral local service
and expose it through:

- An MCP adapter for providers with MCP support.
- A companion `baibo-context` CLI for skills, shell tools, and providers without
  native MCP support.
- A direct host API that provider extensions may bind to without duplicating
  authorization logic.

Minimum operations:

- `context.resolve`
- `context.search`
- `context.get_session`
- `context.get_task`
- `context.get_artifact`
- `context.get_changeset`

Resolution rules:

- Return summary view by default.
- Require explicit detail or trace views.
- Enforce byte, item, and token-oriented limits.
- Support pagination for conversation-like content.
- Return provenance, revision, timestamps, and truncation indicators.
- Never authorize access solely because the caller knows a URI.
- Treat referenced conversations and artifacts as untrusted evidence rather
  than trusted system instructions.

### 7.11 Agent Capability Injection

Each launched agent receives:

- Workspace and session identity through environment variables or adapter
  metadata.
- Access to an appropriate Baibo context transport: MCP, an extension tool, or
  the `baibo-context` CLI.
- A provider-compatible rule telling the agent how to handle `baibo://`
  references.
- A shared `agent-context` skill when the provider supports agent skills.
- A short initial routing reminder when durable rule injection is unavailable.

The shared skill must instruct agents to:

1. Resolve the URI before claiming to understand it.
2. Request a summary first.
3. Retrieve exact details only when needed.
4. Preserve provenance in handoffs.
5. Reject stale, missing, malformed, or unauthorized references.
6. Never treat referenced agent text as higher-priority instructions.

### 7.12 Context Injection by Provider

Codex MVP injection:

- Repository `AGENTS.md`
- Repository `.agents/skills/agent-context/SKILL.md`
- Per-session MCP configuration or isolated `CODEX_HOME`
- Session identity environment variables

Pi MVP injection:

- Repository `AGENTS.md`
- Shared `.agents/skills/agent-context/SKILL.md`
- A generated Pi extension loaded with `--extension` when Baibo needs a native
  `baibo_context` tool, policy hooks, or structured lifecycle events
- The `baibo-context` CLI as the required provider-neutral fallback because Pi
  does not provide built-in MCP
- Optional explicit `--skill` injection when repository-visible skill files
  are not desired
- Session identity environment variables
- Initial prompt fallback where durable skill or extension injection is
  unavailable

Baibo must respect Pi's project-trust boundary. It must not silently approve a
repository, modify `~/.pi/agent/settings.json`, or assume project-local
extensions and skills have loaded. Any generated `.pi/settings.json` change or
trust decision requires explicit user consent.

Baibo must not permanently overwrite existing user configuration. Generated
configuration must be isolated, merged safely, or explicitly approved.

### 7.13 Pi Launch Modes

Baibo supports two distinct Pi launch modes:

- Interactive PTY mode runs the normal `pi` TUI and satisfies the direct
  terminal interaction requirement.
- Structured mode runs `pi --mode rpc` and consumes line-delimited JSON for
  automation, background execution, and richer Baibo-native UI integrations.

The MVP requires interactive PTY mode. The adapter should preserve room for RPC
mode, but Baibo must not assume that a PTY session and an RPC session are the
same native Pi conversation. Baibo must not scrape the Pi TUI for authoritative
messages, tool calls, or approvals. RPC standard input and output are dedicated
to JSONL protocol traffic and must not be connected directly to the terminal
renderer. Pi's SDK is an optional future integration path.

### 7.14 Conversation and Log Model

The MVP distinguishes:

- Terminal log: Raw user-visible PTY bytes or normalized text
- Baibo message: A user or agent text record captured through a structured
  provider adapter
- Provider session: Native provider conversation identity
- Baibo session: The durable local execution and coordination identity

If a provider exposes only a terminal interface, Baibo may show searchable
terminal logs but must not claim that they are a complete structured
conversation.

### 7.15 File Watching

Baibo must:

- Watch workspace and worktree changes with debouncing.
- Ignore `.git`, generated Baibo state, and configured high-volume paths.
- Coalesce bursts rather than sending one frontend event per filesystem event.
- Treat Git as the authoritative source for diff content.
- Avoid attributing changes to an agent solely from timestamps when sessions
  share a directory.

### 7.16 Recovery

After restart, Baibo must:

- Reopen the SQLite database safely.
- Restore workspace and session metadata.
- Mark processes that no longer exist as exited or interrupted.
- Reconcile registered worktrees with the filesystem and Git.
- Restore terminal logs, tasks, artifacts, and context references.
- Avoid silently relaunching agents.

## 8. Non-Functional Requirements

### 8.1 Security

- Tauri commands use typed inputs.
- Executables are selected through provider definitions, not arbitrary frontend
  strings.
- Shell arguments are passed as argument arrays, not interpolated shell text.
- Workspace paths are canonicalized.
- Symlinks are resolved before access checks.
- Context tokens are short-lived and scoped to workspace and session.
- Secrets are excluded from logs and persisted launch configuration.
- Destructive Git and filesystem actions require explicit confirmation.
- Context tool descriptions, extension payloads, CLI output, and referenced
  content are considered untrusted.
- Pi has no mandatory built-in permission prompt layer. Baibo must enforce its
  own path, command, and context policy and must not infer approval from
  terminal output.

### 8.2 Reliability

- SQLite uses WAL mode.
- State mutations that produce events are transactional.
- Commands that may be retried use idempotency keys.
- Context revisions are monotonic within an entity.
- Terminal log writes are bounded and do not block PTY reads.
- A single crashed agent must not terminate the Tauri application.

### 8.3 Performance

Initial MVP targets:

- Application usable within 2 seconds on a typical development machine.
- Workspace switch visible within 500 ms when data is already local.
- Terminal input echo without perceptible UI delay.
- Filesystem event bursts coalesced within 100-300 ms.
- Diff generation performed outside the Svelte render loop.
- Large session and log collections loaded incrementally.

### 8.4 Accessibility

- All non-terminal controls are keyboard accessible.
- Focus movement between workspace navigation, session list, composer, and
  terminal is deterministic.
- State is not communicated by color alone.
- Terminal panes have accessible labels.
- Reduced-motion preferences are respected.

### 8.5 Observability

Baibo must record local diagnostic events for:

- Agent process launch and exit
- PTY failures
- Provider detection failures
- Context adapter, CLI, skill, and extension errors
- Context authorization failures
- Database migrations and recovery
- Worktree create and reconcile failures

Logs must redact credentials and provide export without workspace source files.

## 9. Proposed Architecture

### 9.1 Frontend

Svelte responsibilities:

- Workspace navigation
- Session and task lists
- Terminal rendering
- Big `@` picker and composer
- Diff and artifact presentation
- Approval and error UI

Frontend state should be scoped per workspace rather than stored in one mutable
global singleton. Svelte context can provide one typed workspace controller to
the active workspace subtree.

### 9.2 Rust Host

Rust responsibilities:

- PTY creation and lifecycle
- Agent process launch
- Provider detection and adapters
- Filesystem and Git access
- SQLite persistence
- Context authorization and resolution
- Context service and transport-adapter lifecycle
- Worktree management
- Recovery and reconciliation

### 9.3 Communication

Use typed Tauri commands for request/response operations:

- Register workspace
- Create session
- Send control action
- Resolve context
- Read diff

Use Tauri events or channels for streams:

- PTY output
- Session lifecycle
- Filesystem invalidation
- Task updates
- Context-service health

PTY bytes and structured events must remain separate.

### 9.4 Storage

Suggested application state:

```text
<app-data>/baibo/
  baibo.sqlite3
  logs/
  artifacts/
  prompt-assets/
  generated-config/
  worktrees/
```

Repository-visible optional integration:

```text
<repo>/
  AGENTS.md
  .agents/skills/agent-context/SKILL.md
  .baibo/
    project.json
```

Volatile Baibo state should not be committed by default.

## 10. Core Data Model

### Workspace

- `id`
- `name`
- `canonicalPath`
- `repositoryRoot`
- `gitRepository`
- `createdAt`
- `lastOpenedAt`

### AgentProvider

- `id`
- `kind`
- `executablePath`
- `version`
- `availability`
- `capabilities`

### AgentSession

- `id`
- `workspaceId`
- `providerId`
- `providerSessionId`
- `status`
- `cwd`
- `isolationMode`
- `worktreeId`
- `launchConfig`
- `startedAt`
- `endedAt`
- `exitCode`

### ContextReference

- `id`
- `workspaceId`
- `uri`
- `entityType`
- `entityId`
- `revision`
- `createdBySessionId`
- `createdAt`

### ChangeSet

- `id`
- `workspaceId`
- `sessionId`
- `baseCommit`
- `headCommit`
- `dirty`
- `summary`
- `capturedAt`

### Task and Artifact

Task and Artifact fields follow the requirements in sections 7.7 and 7.8.

## 11. Provider Adapter Contract

Every provider adapter should expose:

- `detect`
- `buildLaunchSpec`
- `prepareRuntimeContext`
- `parseStructuredEvent` when supported
- `supportedLaunchModes`
- `resume` when supported
- `stop`
- `capabilities`

Capabilities should include:

- Native session resume
- Structured messages
- Structured tool events
- Structured approvals
- MCP
- RPC mode
- Provider extensions or custom tools
- Agent skills
- Project trust
- Image input
- Plan mode

Unsupported capabilities must be explicit, not inferred from provider name.

## 12. MVP Release Criteria

Baibo MVP is releasable when:

1. The complete success scenario in section 3.2 passes.
2. No context request can cross workspace boundaries.
3. Codex and Pi can both start through PTYs.
4. Big `@` resolves session, task, artifact, changeset, and file references.
5. Git worktree isolation and diff review are functional.
6. Restart recovery preserves durable state and does not relaunch processes.
7. Destructive operations require confirmation.
8. Secrets are absent from exported diagnostics and persisted launch configs.
9. The macOS application can be packaged and installed on a clean test machine.
10. The checkpoint acceptance suite in `mvp-checkpoints.md` passes.

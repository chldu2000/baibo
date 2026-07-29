# CP4 Manual Acceptance — Durable Sessions and Recovery

CP4 persists Shell and Agent identity, structured lifecycle events, and bounded
terminal-log indexes. Restart recovery never relaunches a process or claims to
resume the same provider conversation.

## 1. Prepare sessions

1. Register a test workspace containing a recognizable file.
2. Create one Shell, one Codex session, and one Pi session.
3. In each terminal, print a distinct marker and enough ANSI/UTF-8 output to
   identify the session later.
4. Open **详情** for each session and confirm:
   - Shell/Codex/Pi kind and structured status are correct.
   - Agent executable and detected provider version are shown.
   - CWD is the selected workspace.
   - Lifecycle sequence starts with `CREATED`, then `RUNNING`.
   - Log coverage is `COMPLETE`.

## 2. Restart recovery

1. Leave all three processes running and force-quit Baibo.
2. Relaunch Baibo without manually starting Codex, Pi, or a Shell.
3. Confirm the sessions are restored and classified as `INTERRUPTED`.
4. Reopen each terminal view and confirm the retained marker output is replayed.
5. Open details and confirm one ordered `INTERRUPTED` event with reason
   `app_restart` was appended.
6. Quit and relaunch again; confirm no duplicate interrupted event is added.
7. Confirm Baibo did not silently relaunch any provider process.

## 3. Fresh restart semantics

1. Select an interrupted Codex or Pi session and choose **重新开始**.
2. Confirm a new AgentSession and terminal are created and opened.
3. Confirm the new detail view references the previous AgentSession ID.
4. Confirm the UI does not claim that the native provider conversation was
   resumed.

## 4. Log retention

1. In a disposable Shell, generate more than 2 MiB of terminal output.
2. Wait for persistence to settle, close the terminal view, and reopen it.
3. Confirm Baibo remains responsive and only recent output is replayed.
4. Open details and confirm:
   - retained bytes do not exceed 2 MiB,
   - chunk and sequence metadata remain coherent,
   - coverage is `TRUNCATED`.

## 5. Legacy and deletion safety

1. When upgrading a database produced before CP4, confirm `Shell N` records are
   shown as Shell and unidentifiable records are shown as `LEGACY`.
2. Confirm legacy details explain that provider identity was not guessed.
3. Stop and delete a completed Agent session.
4. Confirm its Baibo metadata, lifecycle, and replay log disappear while the
   workspace directory and recognizable file remain.
5. Remove a workspace and confirm the dialog states that Baibo history will be
   removed but repository and Git data will not be deleted.

## 6. Automated validation

Run:

```bash
npm run check
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run tauri build
```

Pass condition: metadata, events, and bounded logs survive restart; stale
processes become interrupted exactly once; no process is automatically
relaunched; workspace files remain untouched.

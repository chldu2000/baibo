# CP1 Workspace Registry Acceptance

## Prerequisites

- Build and launch Baibo with `npm run tauri dev`.
- Prepare two temporary Git repositories, `repo-a` and `repo-b`, each containing
  a recognizable file.

## Manual acceptance

1. Select **添加工作空间**, choose `repo-a`, and verify it appears as `GIT` and
   becomes active.
2. Register `repo-b`, switch between both entries, and verify the header path and
   active marker update.
3. Rename both registrations, quit Baibo, relaunch it, and verify both names and
   the last active workspace are restored.
4. Try to register `repo-a` again and verify Baibo reports
   `duplicate_workspace` without adding a second row.
5. Remove the `repo-a` registration and verify its directory, `.git`, and
   recognizable file remain untouched.
6. Move `repo-b` outside its registered path, then try to switch to it. Verify
   Baibo reports `workspace_unavailable`, preserves the registration, and does
   not change the current active workspace.
7. Register a non-Git directory and verify it appears as `DIR`.

## Automated validation

```bash
npm run check
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build
```

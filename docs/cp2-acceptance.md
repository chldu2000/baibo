# CP2 PTY Terminal Runtime Acceptance

CP2 adds explicitly created macOS login-shell terminals to the CP1 workspace
registry. It does not launch Codex, Pi, or any other agent.

## Preparation

1. Create two temporary workspaces, `A` and `B`, and register both in Baibo.
2. Start the desktop application with `npm run tauri dev`.
3. Keep Activity Monitor or `ps` available for checking child processes.

## Manual acceptance

1. In workspace A, create two terminals. In workspace B, create one terminal.
   Run `pwd` in each and confirm the selected workspace directory is used.
2. Run the following in separate terminals and confirm output remains isolated:

   ```sh
   printf '\033[31mANSI red\033[0m 中文 🚀\n'
   ```

3. Type Chinese text with an IME, select terminal text, copy and paste it, then
   run `cat` and press Ctrl+C. Confirm the shell receives the interrupt and the
   rest of the application remains responsive.
4. Open `vim` or another full-screen TUI. Resize the window repeatedly and
   confirm the display reflows without duplicated resize requests or overlays.
5. Close a running terminal tab with `×`. Confirm the process continues, then
   select the same session from the left session list (or the compact session
   picker at 960 px width) and confirm its recent output is replayed.
6. Stop one running terminal. Confirm its status becomes `STOPPED` and every
   other terminal remains interactive. Delete the stopped record and confirm no
   workspace files were removed.
7. Run `exit 7` and confirm the tab reports `EXITED 7`. Force another shell to
   terminate by signal and confirm Baibo stays open with a failed exit state.
8. Leave a shell running and restart Baibo. Confirm the prior record is
   `INTERRUPTED`, its bounded recent log can be opened, and no process is
   automatically relaunched.
9. Try to remove a workspace with a running terminal. Confirm Baibo reports
   `workspace_has_running_terminals`; stop the terminal and retry.
10. Toggle screen-reader mode and enhanced contrast. Confirm keyboard focus is
    visible and status text does not rely on color alone.

## Automated validation

Run from the repository root:

```sh
npm run check
npm run lint
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build
```

The final Tauri build requires the normal macOS signing/toolchain environment.

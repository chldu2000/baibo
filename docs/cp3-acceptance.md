# CP3 手工验收：Codex / Pi Agent Adapters

CP3 在 CP2 的 PTY 运行时上增加 Codex 与 Pi 的登录环境检测和内存态 AgentSession。它不会安装 CLI、切换 NVM 版本、解析 TUI 内容、持久化 AgentSession，或修改 Pi trust/settings。

## 准备

1. 在 macOS 登录 Shell 中确认 `command -v codex`、`codex --version`、`command -v pi` 和 `pi --version`。两者必须同时存在于同一个登录环境的 `PATH`。
2. 若 CLI 仅安装在非活动 NVM 版本中，启动 Baibo 后确认顶部和“新建会话”对话框给出可恢复诊断；修正登录环境后使用 `REFRESH`。
3. 注册两个不同工作空间 A/B，并在其中准备可识别文件。

## Provider 与启动

1. 顶部确认 Codex/Pi 的版本与可用状态；刷新期间按钮禁用。
2. 打开“新建会话”，分别创建 Shell、Codex 和 Pi。
3. 确认统一会话列表显示 `SHELL`、`CODEX`、`PI` 文本标记，终端标签和状态可用键盘操作。
4. 在 Codex/Pi 中确认当前目录是所选工作空间；ANSI、中文/emoji、IME、选择、复制粘贴和 resize 正常。
5. 在 A/B 各创建 Agent，确认输入、输出、Stop 和状态不会跨工作空间或跨进程影响。

## 生命周期

1. Stop 一个 Agent，确认其他 Agent 和 Shell 继续运行。
2. 让一个 Agent 正常退出或异常退出，确认 Baibo 保持运行并显示 `EXITED n` 或 `FAILED`。
3. 对已结束 Agent 选择“重新开始”，确认生成新的 AgentSession/terminal，旧记录保留，且 UI 不声称恢复了原 provider conversation。
4. 删除已结束 Agent，确认只删除该终端记录和回放日志，不触碰工作空间文件。
5. 关闭终端标签后确认进程继续运行，并可从会话列表重新打开。
6. 重启 Baibo，确认旧 PTY 记录按 CP2 规则显示为 `INTERRUPTED`；CP3 不恢复内存 AgentSession，也不自动重启 provider。

## Pi trust 与 RPC

1. 在工作空间创建需要 trust 的 `.pi/settings.json`、`.pi/extensions` 或祖先 `.agents/skills`。
2. 选择 Pi 前确认对话框显示只读 trust 状态；未决状态说明决定将在 Pi TUI 中完成。
3. 启动 Pi，确认 Baibo 未传 `--approve`/`--no-approve`，并由 Pi 原生 TUI 处理 trust。
4. 验收前后比较 `~/.pi/agent/trust.json` 和 settings；除用户在 Pi TUI 中主动操作外，Baibo 不应写入这些文件。
5. 通过 Rust 自动测试或本地诊断调用 `run_pi_rpc_probe`；确认 `get_state` 成功、JSONL 不进入 xterm，且临时配置和进程被清理。

## 自动检查

```sh
npm run check
npm run lint
npm test
npm run build

cd src-tauri
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cd ..

npm run tauri build
```

通过标准：Codex/Pi 仅从当前登录环境检测并以绝对 executable、参数数组和工作空间 CWD 直接启动；敏感环境不进入前端、SQLite 或日志；Pi trust 保持只读；AgentSession 在 CP3 仅存在于内存。

<script lang="ts">
	import { onMount } from 'svelte';

	import { getAppInfo } from '$lib/ipc/app';

	let hostStatus = $state('正在连接桌面主进程…');

	onMount(async () => {
		try {
			const app = await getAppInfo();
			hostStatus = `${app.name} ${app.version} · Tauri 主进程已连接`;
		} catch {
			hostStatus = '浏览器预览模式 · 启动 Tauri 后连接主进程';
		}
	});
</script>

<svelte:head>
	<title>Baibo</title>
	<meta name="description" content="Baibo — local-first workspace for coordinating coding agents" />
</svelte:head>

<div class="app-shell">
	<aside class="sidebar" aria-label="工作空间导航">
		<div class="brand">
			<span class="brand-mark" aria-hidden="true">百</span>
			<div>
				<strong>Baibo</strong>
				<small>Agent Workspace</small>
			</div>
		</div>

		<nav aria-label="主导航">
			<button class="nav-item active" type="button">
				<span>工作空间</span>
				<kbd>⌘1</kbd>
			</button>
			<button class="nav-item" type="button">
				<span>任务</span>
				<kbd>⌘2</kbd>
			</button>
		</nav>

		<div class="workspace-list">
			<p class="section-label">当前工作空间</p>
			<button class="workspace active" type="button">
				<span class="status-dot"></span>
				<div>
					<strong>agent-context</strong>
					<small>2 agents · local</small>
				</div>
			</button>
		</div>

		<button class="add-workspace" type="button">＋ 添加工作空间</button>
	</aside>

	<main>
		<header class="topbar">
			<div>
				<p class="eyebrow">WORKSPACE</p>
				<h1>agent-context</h1>
			</div>
			<div class="host-status"><span></span>{hostStatus}</div>
		</header>

		<section class="workspace-grid" aria-label="工作区概览">
			<article class="panel sessions-panel">
				<div class="panel-heading">
					<div>
						<p class="eyebrow">SESSIONS</p>
						<h2>Agent 会话</h2>
					</div>
					<button type="button">＋ 新建会话</button>
				</div>

				<div class="agent-card active">
					<div class="agent-icon">C</div>
					<div class="agent-detail">
						<strong>Codex</strong>
						<small>主工作区 · 运行中</small>
					</div>
					<span class="agent-state">RUNNING</span>
				</div>

				<div class="agent-card">
					<div class="agent-icon pi">π</div>
					<div class="agent-detail">
						<strong>Pi Agent</strong>
						<small>等待启动</small>
					</div>
					<span class="agent-state idle">IDLE</span>
				</div>
			</article>

			<article class="panel terminal-panel">
				<div class="terminal-tabs">
					<button class="active" type="button"><span></span> Codex</button>
					<button type="button">＋</button>
				</div>
				<div class="terminal" aria-label="终端占位区域">
					<p><span class="prompt">baibo</span> <span class="path">~/agent-context</span></p>
					<p>Terminal runtime will be connected in CP2.</p>
					<p class="cursor-line"><span>❯</span><i></i></p>
				</div>
				<footer>
					<span>PTY · UTF-8</span>
					<span>main</span>
				</footer>
			</article>
		</section>
	</main>
</div>

<script lang="ts">
	import { onMount } from 'svelte';

	import type { Workspace } from '$lib/domain/workspace';
	import { getAppInfo } from '$lib/ipc/app';
	import { getWorkspaceController } from '$lib/state/workspace-context';

	const controller = getWorkspaceController();

	let hostStatus = $state('正在连接桌面主进程…');
	let selectedWorkspace = $state<Workspace | null>(null);
	let renameValue = $state('');
	let renameDialog: HTMLDialogElement;
	let removeDialog: HTMLDialogElement;

	const activeWorkspace = $derived(
		controller.snapshot.workspaces.find(
			(workspace) => workspace.id === controller.snapshot.activeWorkspaceId
		) ?? null
	);

	onMount(async () => {
		try {
			const app = await getAppInfo();
			hostStatus = `${app.name} ${app.version} · Tauri 主进程已连接`;
		} catch {
			hostStatus = '浏览器预览模式 · 启动 Tauri 后连接主进程';
		}
	});

	function showRenameDialog(workspace: Workspace) {
		controller.clearError();
		selectedWorkspace = workspace;
		renameValue = workspace.name;
		renameDialog.showModal();
	}

	function showRemoveDialog(workspace: Workspace) {
		controller.clearError();
		selectedWorkspace = workspace;
		removeDialog.showModal();
	}

	function guardDialogCancel(event: Event) {
		if (controller.busy) event.preventDefault();
	}

	async function submitRename(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedWorkspace) return;
		if (await controller.rename(selectedWorkspace.id, renameValue)) {
			renameDialog.close();
			selectedWorkspace = null;
		}
	}

	async function confirmRemove() {
		if (!selectedWorkspace) return;
		if (await controller.remove(selectedWorkspace.id)) {
			removeDialog.close();
			selectedWorkspace = null;
		}
	}
</script>

<svelte:head>
	<title>Baibo · 工作空间</title>
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
			<button class="nav-item active" type="button" aria-current="page">
				<span>工作空间</span>
				<kbd>⌘1</kbd>
			</button>
			<button class="nav-item" type="button" disabled title="将在后续 checkpoint 提供">
				<span>任务</span>
				<kbd>⌘2</kbd>
			</button>
		</nav>

		<section class="workspace-list" aria-labelledby="workspace-list-title">
			<p class="section-label" id="workspace-list-title">WORKSPACES</p>

			{#if controller.loading}
				<p class="sidebar-message" aria-live="polite">正在载入工作空间…</p>
			{:else if controller.snapshot.workspaces.length === 0}
				<p class="sidebar-message">尚未注册工作空间</p>
			{:else}
				<div class="workspace-items">
					{#each controller.snapshot.workspaces as workspace (workspace.id)}
						<div
							class:active={workspace.id === controller.snapshot.activeWorkspaceId}
							class="workspace-row"
						>
							<button
								class="workspace-select"
								type="button"
								disabled={controller.busy}
								aria-current={workspace.id === controller.snapshot.activeWorkspaceId
									? 'page'
									: undefined}
								onclick={() => controller.open(workspace.id)}
							>
								<span class="status-mark" aria-hidden="true">
									{workspace.id === controller.snapshot.activeWorkspaceId ? '●' : '○'}
								</span>
								<span class="workspace-copy">
									<strong>{workspace.name}</strong>
									<small title={workspace.canonicalPath}>{workspace.canonicalPath}</small>
								</span>
								<span class="workspace-kind">{workspace.gitRepository ? 'GIT' : 'DIR'}</span>
							</button>
							<div class="workspace-actions" aria-label={`${workspace.name} 操作`}>
								<button
									type="button"
									title={`重命名 ${workspace.name}`}
									aria-label={`重命名 ${workspace.name}`}
									disabled={controller.busy}
									onclick={() => showRenameDialog(workspace)}>REN</button
								>
								<button
									type="button"
									class="danger-action"
									title={`移除 ${workspace.name} 的登记`}
									aria-label={`移除 ${workspace.name} 的登记`}
									disabled={controller.busy}
									onclick={() => showRemoveDialog(workspace)}>DEL</button
								>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>

		<button
			class="add-workspace"
			type="button"
			disabled={controller.busy}
			onclick={() => controller.register()}
		>
			{controller.pendingAction === 'registering' ? '[…] 选择中' : '[+] 添加工作空间'}
		</button>
	</aside>

	<main>
		<header class="topbar">
			<div class="topbar-title">
				<p class="eyebrow">WORKSPACE</p>
				<h1>{activeWorkspace?.name ?? '未选择工作空间'}</h1>
				{#if activeWorkspace}
					<p class="workspace-path" title={activeWorkspace.canonicalPath}>
						{activeWorkspace.canonicalPath}
					</p>
				{/if}
			</div>
			<div class="host-status"><span aria-hidden="true"></span>{hostStatus}</div>
		</header>

		{#if controller.error && !selectedWorkspace}
			<div class="error-banner" role="alert">
				<span><strong>[{controller.error.code}]</strong> {controller.error.message}</span>
				<button type="button" aria-label="关闭错误提示" onclick={controller.clearError}>×</button>
			</div>
		{/if}

		{#if controller.loading}
			<section class="center-state" aria-live="polite">
				<p class="ascii-mark" aria-hidden="true">[···]</p>
				<h2>正在载入本地注册表</h2>
			</section>
		{:else if !activeWorkspace}
			<section class="center-state">
				<pre class="ascii-art" aria-hidden="true">┌─ BAIBO ─┐
│  &gt;_     │
└─────────┘</pre>
				<p class="eyebrow">NO WORKSPACE</p>
				<h2>添加第一个本地工作空间</h2>
				<p>选择 Git 仓库或普通目录。Baibo 只登记路径，不移动或复制其中的文件。</p>
				<button type="button" disabled={controller.busy} onclick={() => controller.register()}>
					[+] 选择目录
				</button>
			</section>
		{:else}
			<section class="workspace-grid" aria-label={`${activeWorkspace.name} 工作区概览`}>
				<article class="panel sessions-panel">
					<div class="panel-heading">
						<div>
							<p class="eyebrow">SESSIONS</p>
							<h2>Agent 会话</h2>
						</div>
						<span class="checkpoint-label">CP2 / CP3</span>
					</div>
					<div class="panel-empty">
						<p class="ascii-mark" aria-hidden="true">[ EMPTY ]</p>
						<h3>尚无可运行会话</h3>
						<p>PTY 终端将在 CP2 接入，Codex 与 Pi Agent 将在 CP3 接入。</p>
					</div>
				</article>

				<article class="panel terminal-panel">
					<div class="terminal-tabs" aria-label="终端标签">
						<span class="terminal-tab">○ TERMINAL · NOT CONNECTED</span>
					</div>
					<div class="terminal terminal-placeholder" aria-label="终端将在 CP2 提供">
						<p>
							<span class="prompt">baibo</span> <span class="path">{activeWorkspace.name}</span>
						</p>
						<p>Workspace registry connected.</p>
						<p>Terminal runtime will be connected in CP2.</p>
						<p class="cursor-line"><span>❯</span><i></i></p>
					</div>
					<footer>
						<span>{activeWorkspace.gitRepository ? 'GIT REPOSITORY' : 'LOCAL DIRECTORY'}</span>
						<span>CP1</span>
					</footer>
				</article>
			</section>
		{/if}
	</main>
</div>

<dialog
	bind:this={renameDialog}
	aria-labelledby="rename-title"
	oncancel={guardDialogCancel}
	onclose={() => (selectedWorkspace = null)}
>
	<form class="dialog-form" onsubmit={submitRename}>
		<div>
			<p class="eyebrow">RENAME</p>
			<h2 id="rename-title">重命名工作空间</h2>
		</div>
		<label for="workspace-name">显示名称</label>
		<input
			id="workspace-name"
			name="workspace-name"
			bind:value={renameValue}
			minlength="1"
			maxlength="128"
			required
			autocomplete="off"
		/>
		{#if controller.error}
			<div class="dialog-error" role="alert">
				<strong>[{controller.error.code}]</strong>
				<span>{controller.error.message}</span>
			</div>
		{/if}
		<div class="dialog-actions">
			<button type="button" disabled={controller.busy} onclick={() => renameDialog.close()}>
				取消
			</button>
			<button class="primary-action" type="submit" disabled={controller.busy}>
				{controller.pendingAction === 'renaming' ? '保存中…' : '保存'}
			</button>
		</div>
	</form>
</dialog>

<dialog
	bind:this={removeDialog}
	aria-labelledby="remove-title"
	oncancel={guardDialogCancel}
	onclose={() => (selectedWorkspace = null)}
>
	<div class="dialog-form">
		<div>
			<p class="eyebrow danger-text">REMOVE REGISTRATION</p>
			<h2 id="remove-title">移除“{selectedWorkspace?.name}”的登记？</h2>
		</div>
		<p>只会从 Baibo 移除登记，不会删除目录、仓库、<code>.git</code> 或其中任何文件。</p>
		{#if controller.error}
			<div class="dialog-error" role="alert">
				<strong>[{controller.error.code}]</strong>
				<span>{controller.error.message}</span>
			</div>
		{/if}
		<div class="dialog-actions">
			<button type="button" disabled={controller.busy} onclick={() => removeDialog.close()}>
				取消
			</button>
			<button
				class="danger-button"
				type="button"
				disabled={controller.busy}
				onclick={confirmRemove}
			>
				{controller.pendingAction === 'removing' ? '移除中…' : '移除登记'}
			</button>
		</div>
	</div>
</dialog>

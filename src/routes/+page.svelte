<script lang="ts">
	import { onMount, untrack } from 'svelte';

	import TerminalView from '$lib/components/TerminalView.svelte';
	import type { TerminalSession } from '$lib/domain/terminal';
	import type { Workspace } from '$lib/domain/workspace';
	import { getAppInfo } from '$lib/ipc/app';
	import { getTerminalController } from '$lib/state/terminal-context';
	import { getWorkspaceController } from '$lib/state/workspace-context';

	const controller = getWorkspaceController();
	const terminals = getTerminalController();

	let hostStatus = $state('正在连接桌面主进程…');
	let selectedWorkspace = $state<Workspace | null>(null);
	let selectedTerminal = $state<TerminalSession | null>(null);
	let renameValue = $state('');
	let screenReaderMode = $state(false);
	let enhancedContrast = $state(false);
	let renameDialog: HTMLDialogElement;
	let removeDialog: HTMLDialogElement;
	let deleteTerminalDialog: HTMLDialogElement;
	let terminalStage: HTMLElement | undefined;

	const activeWorkspace = $derived(
		controller.snapshot.workspaces.find(
			(workspace) => workspace.id === controller.snapshot.activeWorkspaceId
		) ?? null
	);
	const terminalState = $derived(activeWorkspace ? terminals.state(activeWorkspace.id) : null);
	const visibleTerminals = $derived(
		terminalState
			? terminalState.sessions.filter((session) => !terminalState.closedTerminalIds.has(session.id))
			: []
	);
	const activeTerminal = $derived(
		terminalState?.sessions.find((session) => session.id === terminalState.activeTerminalId) ?? null
	);

	$effect(() => {
		const workspaceId = activeWorkspace?.id ?? null;
		if (workspaceId) untrack(() => void terminals.load(workspaceId));
	});

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
		if (controller.busy || terminals.busy) event.preventDefault();
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

	async function createTerminal() {
		if (!activeWorkspace) return;
		const width = terminalStage?.clientWidth ?? 720;
		const height = terminalStage?.clientHeight ?? 420;
		const cols = Math.max(2, Math.min(500, Math.floor((width - 24) / 8)));
		const rows = Math.max(1, Math.min(200, Math.floor((height - 20) / 17)));
		await terminals.create(activeWorkspace.id, cols, rows);
	}

	function showDeleteTerminalDialog(session: TerminalSession) {
		terminals.clearError();
		selectedTerminal = session;
		deleteTerminalDialog.showModal();
	}

	async function confirmDeleteTerminal() {
		if (!selectedTerminal) return;
		await terminals.delete(selectedTerminal.workspaceId, selectedTerminal.id);
		if (!terminals.error) {
			deleteTerminalDialog.close();
			selectedTerminal = null;
		}
	}

	function isRunning(session: TerminalSession): boolean {
		return session.status === 'starting' || session.status === 'running';
	}

	function terminalStatus(session: TerminalSession): string {
		switch (session.status) {
			case 'starting':
				return 'STARTING';
			case 'running':
				return 'RUNNING';
			case 'exited':
				return `EXITED ${session.exitCode ?? 0}`;
			case 'failed':
				return 'FAILED';
			case 'stopped':
				return 'STOPPED';
			case 'interrupted':
				return 'INTERRUPTED';
		}
	}

	function selectTerminalFromMenu(event: Event) {
		if (!activeWorkspace) return;
		const terminalId = (event.currentTarget as HTMLSelectElement).value;
		if (terminalId) terminals.select(activeWorkspace.id, terminalId);
	}

	function terminalStageAttachment(node: HTMLElement) {
		terminalStage = node;
		return () => {
			if (terminalStage === node) terminalStage = undefined;
		};
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

		{#if (controller.error && !selectedWorkspace) || (terminals.error && !selectedTerminal)}
			{@const error = controller.error ?? terminals.error}
			<div class="error-banner" role="alert">
				{#if error}
					<span><strong>[{error.code}]</strong> {error.message}</span>
				{/if}
				<button
					type="button"
					aria-label="关闭错误提示"
					onclick={() => {
						controller.clearError();
						terminals.clearError();
					}}>×</button
				>
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
			<section class="workspace-grid" aria-label={`${activeWorkspace.name} 终端工作区`}>
				<aside class="panel sessions-panel" aria-label="终端会话">
					<div class="panel-heading">
						<div>
							<p class="eyebrow">TERMINALS</p>
							<h2>Shell 会话</h2>
						</div>
						<button
							class="compact-action"
							type="button"
							disabled={terminals.busy}
							onclick={createTerminal}
						>
							{terminals.pendingAction === 'creating' ? '[…]' : '[+] 新建'}
						</button>
					</div>
					{#if terminals.pendingAction === 'loading' && terminalState?.sessions.length === 0}
						<p class="session-message" aria-live="polite">正在载入终端记录…</p>
					{:else if terminalState?.sessions.length === 0}
						<div class="panel-empty session-empty">
							<p class="ascii-mark" aria-hidden="true">[ NO PTY ]</p>
							<h3>尚未创建终端</h3>
							<p>显式创建一个登录 Shell；不会自动启动 Agent。</p>
						</div>
					{:else}
						<div class="session-items">
							{#each terminalState?.sessions ?? [] as session (session.id)}
								<button
									class:active={session.id === terminalState?.activeTerminalId}
									class="session-row"
									type="button"
									onclick={() => terminals.select(activeWorkspace.id, session.id)}
								>
									<span aria-hidden="true">{isRunning(session) ? '●' : '○'}</span>
									<span>
										<strong>{session.title}</strong>
										<small>{terminalStatus(session)}</small>
									</span>
								</button>
							{/each}
						</div>
					{/if}
				</aside>

				<article class="panel terminal-panel" {@attach terminalStageAttachment}>
					<div class="terminal-tabs" role="tablist" aria-label="已打开的终端标签">
						{#each visibleTerminals as session (session.id)}
							<div
								class:active={session.id === terminalState?.activeTerminalId}
								class="terminal-tab"
							>
								<button
									type="button"
									role="tab"
									aria-selected={session.id === terminalState?.activeTerminalId}
									onclick={() => terminals.select(activeWorkspace.id, session.id)}
								>
									{session.title} · {terminalStatus(session)}
								</button>
								<button
									class="close-tab"
									type="button"
									aria-label={`关闭 ${session.title} 视图，进程继续运行`}
									title="关闭视图（进程继续运行）"
									onclick={() => terminals.closeView(activeWorkspace.id, session.id)}
								>
									×
								</button>
							</div>
						{/each}
						{#if visibleTerminals.length === 0}
							<span class="terminal-tab-placeholder">○ NO OPEN TERMINAL</span>
						{/if}
					</div>
					<div class="terminal-toolbar">
						<label class="compact-session-picker">
							<span>会话</span>
							<select
								value={activeTerminal?.id ?? ''}
								aria-label="选择或重新打开终端会话"
								onchange={selectTerminalFromMenu}
							>
								<option value="" disabled>选择终端</option>
								{#each terminalState?.sessions ?? [] as session (session.id)}
									<option value={session.id}>{session.title} · {terminalStatus(session)}</option>
								{/each}
							</select>
						</label>
						<div class="terminal-a11y">
							<label>
								<input type="checkbox" bind:checked={screenReaderMode} />
								屏幕阅读器模式
							</label>
							<label>
								<input type="checkbox" bind:checked={enhancedContrast} />
								增强对比度
							</label>
						</div>
						{#if activeTerminal}
							<div class="terminal-actions">
								{#if isRunning(activeTerminal)}
									<button
										type="button"
										disabled={terminals.busy}
										onclick={() => terminals.stop(activeWorkspace.id, activeTerminal.id)}
									>
										{terminals.pendingAction === 'stopping' ? '停止中…' : 'Stop'}
									</button>
								{:else}
									<button
										class="danger-action"
										type="button"
										disabled={terminals.busy}
										onclick={() => showDeleteTerminalDialog(activeTerminal)}
									>
										删除记录
									</button>
								{/if}
							</div>
						{/if}
					</div>
					<div class="terminal-viewport">
						{#if activeTerminal && !terminalState?.closedTerminalIds.has(activeTerminal.id)}
							{#key `${activeTerminal.id}:${screenReaderMode}:${enhancedContrast}`}
								<TerminalView
									session={activeTerminal}
									{screenReaderMode}
									minimumContrastRatio={enhancedContrast ? 7 : 4.5}
									controller={terminals}
								/>
							{/key}
						{:else}
							<div class="terminal-empty">
								<p class="ascii-mark" aria-hidden="true">&gt;_</p>
								<h3>没有打开的终端视图</h3>
								<p>从左侧重新打开已有会话，或新建登录 Shell。</p>
								<button type="button" disabled={terminals.busy} onclick={createTerminal}>
									[+] 新建终端
								</button>
							</div>
						{/if}
					</div>
					<footer>
						<span>{activeWorkspace.gitRepository ? 'GIT REPOSITORY' : 'LOCAL DIRECTORY'}</span>
						<span>{activeTerminal ? `${activeTerminal.cols}×${activeTerminal.rows}` : 'CP2'}</span>
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
	bind:this={deleteTerminalDialog}
	aria-labelledby="delete-terminal-title"
	oncancel={guardDialogCancel}
	onclose={() => (selectedTerminal = null)}
>
	<div class="dialog-form">
		<div>
			<p class="eyebrow danger-text">DELETE TERMINAL RECORD</p>
			<h2 id="delete-terminal-title">删除“{selectedTerminal?.title}”的记录？</h2>
		</div>
		<p>将删除该终端的会话记录和最多 2 MiB 回放日志，不会删除工作空间中的任何文件。</p>
		{#if terminals.error}
			<div class="dialog-error" role="alert">
				<strong>[{terminals.error.code}]</strong>
				<span>{terminals.error.message}</span>
			</div>
		{/if}
		<div class="dialog-actions">
			<button type="button" disabled={terminals.busy} onclick={() => deleteTerminalDialog.close()}
				>取消</button
			>
			<button
				class="danger-button"
				type="button"
				disabled={terminals.busy}
				onclick={confirmDeleteTerminal}
			>
				{terminals.pendingAction === 'deleting' ? '删除中…' : '删除记录与日志'}
			</button>
		</div>
	</div>
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

<script lang="ts">
	import { onMount } from 'svelte';

	import favicon from '$lib/assets/favicon.svg';
	import { WorkspaceController } from '$lib/state/workspace.svelte';
	import { setWorkspaceController } from '$lib/state/workspace-context';
	import { TerminalController } from '$lib/state/terminal.svelte';
	import { setTerminalController } from '$lib/state/terminal-context';
	import '../app.css';

	let { children } = $props();

	const workspaceController = new WorkspaceController();
	const terminalController = new TerminalController();
	setWorkspaceController(workspaceController);
	setTerminalController(terminalController);

	onMount(() => {
		void workspaceController.load();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

<script lang="ts">
	import { onMount } from 'svelte';

	import favicon from '$lib/assets/favicon.svg';
	import { WorkspaceController } from '$lib/state/workspace.svelte';
	import { setWorkspaceController } from '$lib/state/workspace-context';
	import '../app.css';

	let { children } = $props();

	const workspaceController = new WorkspaceController();
	setWorkspaceController(workspaceController);

	onMount(() => {
		void workspaceController.load();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

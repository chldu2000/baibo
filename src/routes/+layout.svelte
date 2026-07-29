<script lang="ts">
	import { onMount } from 'svelte';

	import favicon from '$lib/assets/favicon.svg';
	import { WorkspaceController } from '$lib/state/workspace.svelte';
	import { setWorkspaceController } from '$lib/state/workspace-context';
	import { TerminalController } from '$lib/state/terminal.svelte';
	import { setTerminalController } from '$lib/state/terminal-context';
	import { ProviderController } from '$lib/state/provider.svelte';
	import { setProviderController } from '$lib/state/provider-context';
	import { AgentController } from '$lib/state/agent.svelte';
	import { setAgentController } from '$lib/state/agent-context';
	import '../app.css';

	let { children } = $props();

	const workspaceController = new WorkspaceController();
	const terminalController = new TerminalController();
	const providerController = new ProviderController();
	const agentController = new AgentController();
	setWorkspaceController(workspaceController);
	setTerminalController(terminalController);
	setProviderController(providerController);
	setAgentController(agentController);

	onMount(() => {
		void workspaceController.load();
		void providerController.load();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

<script lang="ts">
	import { FitAddon } from '@xterm/addon-fit';
	import { Terminal } from '@xterm/xterm';
	import '@xterm/xterm/css/xterm.css';
	import { untrack } from 'svelte';

	import type { TerminalSession } from '$lib/domain/terminal';
	import { createTerminalChannels, terminalApi } from '$lib/ipc/terminal';
	import type { TerminalController } from '$lib/state/terminal.svelte';
	import { TerminalInputBatcher } from '$lib/terminal/input-batcher';
	import { TerminalResizeQueue } from '$lib/terminal/resize-queue';

	let {
		session,
		controller,
		screenReaderMode = false,
		minimumContrastRatio = 4.5
	}: {
		session: TerminalSession;
		controller: TerminalController;
		screenReaderMode?: boolean;
		minimumContrastRatio?: number;
	} = $props();

	const encoder = new TextEncoder();

	function terminalAttachment(node: HTMLElement) {
		const terminalSession = untrack(() => session);
		const terminalScreenReaderMode = untrack(() => screenReaderMode);
		const terminalMinimumContrastRatio = untrack(() => minimumContrastRatio);
		const terminal = new Terminal({
			allowProposedApi: false,
			convertEol: false,
			cursorBlink: true,
			fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
			fontSize: 13,
			lineHeight: 1.25,
			minimumContrastRatio: terminalMinimumContrastRatio,
			screenReaderMode: terminalScreenReaderMode,
			scrollback: 10_000,
			theme: {
				background: '#11151b',
				foreground: '#e8edf5',
				cursor: '#a7b0bd',
				cursorAccent: '#11151b',
				selectionBackground: '#253d5a'
			}
		});
		const fit = new FitAddon();
		terminal.loadAddon(fit);
		terminal.open(node);

		let disposed = false;
		let subscriptionId: string | null = null;
		let resizeFrame = 0;
		let lastCols = 0;
		let lastRows = 0;
		const inputBatcher = new TerminalInputBatcher(
			(data) => terminalApi.write(terminalSession.workspaceId, terminalSession.id, data),
			controller.reportError
		);
		const resizeQueue = new TerminalResizeQueue(async (cols, rows) => {
			const updated = await terminalApi.resize(
				terminalSession.workspaceId,
				terminalSession.id,
				cols,
				rows
			);
			if (!disposed) controller.updateSession(updated);
		}, controller.reportError);

		const inputDisposable = terminal.onData((data) => inputBatcher.push(encoder.encode(data)));
		const binaryDisposable = terminal.onBinary((data) =>
			inputBatcher.push(Uint8Array.from(data, (character) => character.charCodeAt(0) & 0xff))
		);
		const resizeObserver = new ResizeObserver(() => {
			cancelAnimationFrame(resizeFrame);
			resizeFrame = requestAnimationFrame(() => {
				if (disposed || node.clientWidth === 0 || node.clientHeight === 0) return;
				fit.fit();
				if (terminal.cols === lastCols && terminal.rows === lastRows) return;
				lastCols = terminal.cols;
				lastRows = terminal.rows;
				resizeQueue.request(terminal.cols, terminal.rows);
			});
		});
		resizeObserver.observe(node);

		const channels = createTerminalChannels((data) => {
			if (!disposed) terminal.write(data);
		}, controller.handleEvent);
		void terminalApi
			.attach(terminalSession.workspaceId, terminalSession.id, channels)
			.then((attachment) => {
				if (disposed) {
					return terminalApi.detach(
						terminalSession.workspaceId,
						terminalSession.id,
						attachment.subscriptionId
					);
				}
				subscriptionId = attachment.subscriptionId;
				controller.updateSession(attachment.session);
				terminal.focus();
			})
			.catch(controller.reportError);

		return () => {
			inputBatcher.dispose();
			resizeQueue.dispose();
			disposed = true;
			cancelAnimationFrame(resizeFrame);
			resizeObserver.disconnect();
			inputDisposable.dispose();
			binaryDisposable.dispose();
			terminal.dispose();
			if (subscriptionId) {
				void terminalApi
					.detach(terminalSession.workspaceId, terminalSession.id, subscriptionId)
					.catch(controller.reportError);
			}
		};
	}
</script>

<div
	class="terminal-host"
	role="application"
	aria-label={`${session.title} 交互式终端`}
	{@attach terminalAttachment}
></div>

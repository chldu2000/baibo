import { Channel, invoke } from '@tauri-apps/api/core';

import type { TerminalAttachment, TerminalEvent, TerminalSession } from '$lib/domain/terminal';

export interface TerminalChannels {
	output: Channel<number[] | Uint8Array>;
	events: Channel<TerminalEvent>;
}

export interface TerminalApi {
	list(workspaceId: string): Promise<TerminalSession[]>;
	create(workspaceId: string, cols: number, rows: number): Promise<TerminalSession>;
	attach(
		workspaceId: string,
		terminalId: string,
		channels: TerminalChannels
	): Promise<TerminalAttachment>;
	detach(workspaceId: string, terminalId: string, subscriptionId: string): Promise<void>;
	write(workspaceId: string, terminalId: string, data: Uint8Array): Promise<void>;
	resize(
		workspaceId: string,
		terminalId: string,
		cols: number,
		rows: number
	): Promise<TerminalSession>;
	stop(workspaceId: string, terminalId: string): Promise<TerminalSession>;
	delete(workspaceId: string, terminalId: string): Promise<void>;
}

export const terminalApi: TerminalApi = {
	list: (workspaceId) => invoke('list_terminals', { workspaceId }),
	create: (workspaceId, cols, rows) => invoke('create_terminal', { workspaceId, cols, rows }),
	attach: (workspaceId, terminalId, channels) =>
		invoke('attach_terminal', {
			workspaceId,
			terminalId,
			outputChannel: channels.output,
			eventChannel: channels.events
		}),
	detach: (workspaceId, terminalId, subscriptionId) =>
		invoke('detach_terminal', { workspaceId, terminalId, subscriptionId }),
	write: (workspaceId, terminalId, data) =>
		invoke('write_terminal_input', { workspaceId, terminalId, data: Array.from(data) }),
	resize: (workspaceId, terminalId, cols, rows) =>
		invoke('resize_terminal', { workspaceId, terminalId, cols, rows }),
	stop: (workspaceId, terminalId) => invoke('stop_terminal', { workspaceId, terminalId }),
	delete: (workspaceId, terminalId) => invoke('delete_terminal', { workspaceId, terminalId })
};

export const createTerminalChannels = (
	onOutput: (data: Uint8Array) => void,
	onEvent: (event: TerminalEvent) => void
): TerminalChannels => {
	const output = new Channel<number[] | Uint8Array>();
	output.onmessage = (data) => onOutput(data instanceof Uint8Array ? data : Uint8Array.from(data));
	const events = new Channel<TerminalEvent>();
	events.onmessage = onEvent;
	return { output, events };
};

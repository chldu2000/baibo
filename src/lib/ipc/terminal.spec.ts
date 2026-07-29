import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, channels } = vi.hoisted(() => ({
	invoke: vi.fn(),
	channels: [] as Array<{ onmessage?: (message: unknown) => void }>
}));

vi.mock('@tauri-apps/api/core', () => ({
	invoke,
	Channel: class {
		onmessage?: (message: unknown) => void;

		constructor() {
			channels.push(this);
		}
	}
}));

import { createTerminalChannels, terminalApi } from './terminal';

describe('terminalApi', () => {
	beforeEach(() => {
		invoke.mockReset();
		channels.length = 0;
		invoke.mockResolvedValue(undefined);
	});

	it('uses scoped command names and serializes bytes without text conversion', async () => {
		const output = createTerminalChannels(vi.fn(), vi.fn());

		await terminalApi.list('workspace-a');
		await terminalApi.create('workspace-a', 80, 24);
		await terminalApi.attach('workspace-a', 'terminal-a', output);
		await terminalApi.detach('workspace-a', 'terminal-a', 'subscription-a');
		await terminalApi.write('workspace-a', 'terminal-a', Uint8Array.of(0, 3, 255));
		await terminalApi.resize('workspace-a', 'terminal-a', 120, 40);
		await terminalApi.stop('workspace-a', 'terminal-a');
		await terminalApi.delete('workspace-a', 'terminal-a');
		await terminalApi.detail('workspace-a', 'terminal-a');

		expect(invoke.mock.calls).toEqual([
			['list_terminals', { workspaceId: 'workspace-a' }],
			['create_terminal', { workspaceId: 'workspace-a', cols: 80, rows: 24 }],
			[
				'attach_terminal',
				{
					workspaceId: 'workspace-a',
					terminalId: 'terminal-a',
					outputChannel: output.output,
					eventChannel: output.events
				}
			],
			[
				'detach_terminal',
				{
					workspaceId: 'workspace-a',
					terminalId: 'terminal-a',
					subscriptionId: 'subscription-a'
				}
			],
			[
				'write_terminal_input',
				{ workspaceId: 'workspace-a', terminalId: 'terminal-a', data: [0, 3, 255] }
			],
			[
				'resize_terminal',
				{ workspaceId: 'workspace-a', terminalId: 'terminal-a', cols: 120, rows: 40 }
			],
			['stop_terminal', { workspaceId: 'workspace-a', terminalId: 'terminal-a' }],
			['delete_terminal', { workspaceId: 'workspace-a', terminalId: 'terminal-a' }],
			['get_session_detail', { workspaceId: 'workspace-a', terminalId: 'terminal-a' }]
		]);
	});

	it('normalizes output channel messages to Uint8Array and keeps events structured', () => {
		const onOutput = vi.fn();
		const onEvent = vi.fn();
		createTerminalChannels(onOutput, onEvent);
		const event = { event: 'outputLagged', data: { terminalId: 'terminal-a' } };

		channels[0]?.onmessage?.([0, 3, 255]);
		channels[1]?.onmessage?.(event);

		expect(onOutput).toHaveBeenCalledWith(Uint8Array.of(0, 3, 255));
		expect(onEvent).toHaveBeenCalledWith(event);
	});
});

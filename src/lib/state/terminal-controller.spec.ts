import { describe, expect, it, vi } from 'vitest';

import type { TerminalSession } from '$lib/domain/terminal';
import type { SessionDetail } from '$lib/domain/session';
import type { TerminalApi } from '$lib/ipc/terminal';

import { TerminalController } from './terminal.svelte';

const running = (workspaceId: string, id: string): TerminalSession => ({
	id,
	workspaceId,
	title: `Shell ${id}`,
	shell: '/bin/zsh',
	cwd: `/tmp/${workspaceId}`,
	status: 'running',
	cols: 80,
	rows: 24,
	createdAt: 1,
	startedAt: 2,
	endedAt: null,
	exitCode: null,
	terminationReason: null,
	sessionKind: 'shell'
});

const detail = (workspaceId: string, terminalId: string): SessionDetail => ({
	terminal: running(workspaceId, terminalId),
	agentSession: null,
	lifecycleEvents: [],
	logIndex: {
		terminalId,
		firstSequence: null,
		lastSequence: null,
		chunkCount: 0,
		retainedBytes: 0,
		coverage: 'complete',
		updatedAt: 1
	}
});

function api(overrides: Partial<TerminalApi> = {}): TerminalApi {
	return {
		list: vi.fn().mockResolvedValue([]),
		create: vi.fn().mockResolvedValue(running('workspace-a', 'terminal-a')),
		attach: vi.fn(),
		detach: vi.fn(),
		write: vi.fn(),
		resize: vi.fn(),
		stop: vi.fn().mockResolvedValue(running('workspace-a', 'terminal-a')),
		delete: vi.fn().mockResolvedValue(undefined),
		detail: vi.fn(),
		...overrides
	};
}

describe('TerminalController', () => {
	it('keeps session and active-tab state isolated by workspace', async () => {
		const client = api({
			list: vi
				.fn()
				.mockResolvedValueOnce([running('workspace-a', 'terminal-a')])
				.mockResolvedValueOnce([running('workspace-b', 'terminal-b')])
		});
		const controller = new TerminalController(client);

		await controller.load('workspace-a');
		await controller.load('workspace-b');
		controller.closeView('workspace-a', 'terminal-a');

		expect(controller.state('workspace-a').activeTerminalId).toBeNull();
		expect(controller.state('workspace-a').closedTerminalIds.has('terminal-a')).toBe(true);
		expect(controller.state('workspace-b').activeTerminalId).toBe('terminal-b');
	});

	it('reopens a closed view without creating or stopping its process', async () => {
		const client = api({ list: vi.fn().mockResolvedValue([running('workspace-a', 'terminal-a')]) });
		const controller = new TerminalController(client);
		await controller.load('workspace-a');

		controller.closeView('workspace-a', 'terminal-a');
		controller.select('workspace-a', 'terminal-a');

		expect(controller.state('workspace-a').activeTerminalId).toBe('terminal-a');
		expect(controller.state('workspace-a').closedTerminalIds.has('terminal-a')).toBe(false);
		expect(client.stop).not.toHaveBeenCalled();
	});

	it('applies lifecycle events and reports output lag without storing output bytes', async () => {
		const initial = running('workspace-a', 'terminal-a');
		const controller = new TerminalController(api({ list: vi.fn().mockResolvedValue([initial]) }));
		await controller.load('workspace-a');

		controller.handleEvent({
			event: 'sessionUpdated',
			data: { session: { ...initial, status: 'exited', exitCode: 7, endedAt: 3 } }
		});
		controller.handleEvent({
			event: 'outputLagged',
			data: { terminalId: 'terminal-a' }
		});

		expect(controller.state('workspace-a').sessions[0]?.status).toBe('exited');
		expect(controller.state('workspace-a').sessions[0]?.exitCode).toBe(7);
		expect(controller.error?.code).toBe('terminal_output_lagged');
		expect(controller.state('workspace-a')).not.toHaveProperty('output');
	});

	it('blocks overlapping mutations and recovers from structured errors', async () => {
		let finishCreate: (session: TerminalSession) => void = () => undefined;
		const create = vi.fn(
			() =>
				new Promise<TerminalSession>((resolve) => {
					finishCreate = resolve;
				})
		);
		const stop = vi.fn().mockRejectedValue({
			code: 'terminal_not_running',
			message: '终端进程未在运行'
		});
		const controller = new TerminalController(api({ create, stop }));

		const creating = controller.create('workspace-a', 80, 24);
		await controller.stop('workspace-a', 'terminal-a');
		finishCreate(running('workspace-a', 'terminal-a'));
		await creating;
		await controller.stop('workspace-a', 'terminal-a');

		expect(stop).toHaveBeenCalledTimes(1);
		expect(controller.error).toEqual({
			code: 'terminal_not_running',
			message: '终端进程未在运行'
		});
		expect(controller.pendingAction).toBeNull();
	});

	it('loads the latest requested workspace after an in-flight mutation completes', async () => {
		let finishCreate: (session: TerminalSession) => void = () => undefined;
		const create = vi.fn(
			() =>
				new Promise<TerminalSession>((resolve) => {
					finishCreate = resolve;
				})
		);
		const workspaceB = running('workspace-b', 'terminal-b');
		const list = vi.fn().mockResolvedValue([workspaceB]);
		const controller = new TerminalController(api({ create, list }));

		const creating = controller.create('workspace-a', 80, 24);
		await controller.load('workspace-b');
		expect(list).not.toHaveBeenCalled();
		finishCreate(running('workspace-a', 'terminal-a'));
		await creating;
		await vi.waitFor(() => expect(list).toHaveBeenCalledWith('workspace-b'));
		await vi.waitFor(() =>
			expect(controller.state('workspace-b').activeTerminalId).toBe('terminal-b')
		);
	});

	it('discards stale session-detail responses after workspace changes', async () => {
		let resolveA: ((value: SessionDetail) => void) | undefined;
		let resolveB: ((value: SessionDetail) => void) | undefined;
		const client = api({
			detail: vi.fn(
				(workspaceId: string) =>
					new Promise<SessionDetail>((resolve) => {
						if (workspaceId === 'workspace-a') resolveA = resolve;
						else resolveB = resolve;
					})
			)
		});
		const controller = new TerminalController(client);

		const requestA = controller.loadDetail('workspace-a', 'terminal-a');
		const requestB = controller.loadDetail('workspace-b', 'terminal-b');
		resolveB?.(detail('workspace-b', 'terminal-b'));
		await requestB;
		resolveA?.(detail('workspace-a', 'terminal-a'));
		await requestA;

		expect(controller.detail?.terminal.workspaceId).toBe('workspace-b');
		expect(controller.detailLoading).toBe(false);
	});
});

import { describe, expect, it, vi } from 'vitest';

import type { AgentSession } from '$lib/domain/agent';
import type { AgentApi } from '$lib/ipc/agent';

import { AgentController } from './agent.svelte';

const session = (workspaceId: string, id: string, terminalId: string): AgentSession => ({
	id,
	workspaceId,
	providerId: 'codex',
	providerSessionId: null,
	terminal: {
		id: terminalId,
		workspaceId,
		title: 'Codex 1',
		shell: '/usr/local/bin/codex',
		cwd: `/tmp/${workspaceId}`,
		status: 'running',
		cols: 80,
		rows: 24,
		createdAt: 1,
		startedAt: 2,
		endedAt: null,
		exitCode: null,
		terminationReason: null
	},
	launchMode: 'interactivePty',
	isolationMode: 'workspace',
	restartedFromSessionId: null,
	createdAt: 1
});

function api(overrides: Partial<AgentApi> = {}): AgentApi {
	const created = session('workspace-a', 'agent-a', 'terminal-a');
	return {
		list: vi.fn().mockResolvedValue([]),
		create: vi.fn().mockResolvedValue(created),
		restart: vi.fn().mockResolvedValue({
			...created,
			id: 'agent-b',
			restartedFromSessionId: 'agent-a',
			terminal: { ...created.terminal, id: 'terminal-b' }
		}),
		stop: vi
			.fn()
			.mockResolvedValue({ ...created, terminal: { ...created.terminal, status: 'stopped' } }),
		delete: vi.fn().mockResolvedValue(undefined),
		...overrides
	};
}

describe('AgentController', () => {
	it('keeps sessions partitioned by workspace and decorates terminal IDs', async () => {
		const controller = new AgentController(
			api({
				list: vi
					.fn()
					.mockResolvedValueOnce([session('workspace-a', 'agent-a', 'terminal-a')])
					.mockResolvedValueOnce([session('workspace-b', 'agent-b', 'terminal-b')])
			})
		);

		await controller.load('workspace-a');
		await controller.load('workspace-b');

		expect(controller.byTerminal('workspace-a', 'terminal-a')?.id).toBe('agent-a');
		expect(controller.byTerminal('workspace-a', 'terminal-b')).toBeNull();
	});

	it('creates a fresh session on restart and serializes mutations', async () => {
		const client = api();
		const controller = new AgentController(client);

		const created = await controller.create('workspace-a', 'codex', 80, 24);
		const restarted = await controller.restart('workspace-a', created?.id ?? '', 80, 24);

		expect(restarted?.id).toBe('agent-b');
		expect(restarted?.restartedFromSessionId).toBe('agent-a');
		expect(controller.sessions('workspace-a')).toHaveLength(2);
		expect(client.restart).toHaveBeenCalledWith('workspace-a', 'agent-a', 80, 24);
	});
});

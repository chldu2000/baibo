import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { agentApi } from './agent';
import { providerApi } from './provider';

describe('provider and agent IPC', () => {
	beforeEach(() => {
		invoke.mockReset();
		invoke.mockResolvedValue(undefined);
	});

	it('uses fixed command names and scoped camel-case arguments', async () => {
		await providerApi.list();
		await providerApi.refresh();
		await providerApi.piTrust('workspace-a');
		await providerApi.runPiRpcProbe('workspace-a');
		await agentApi.list('workspace-a');
		await agentApi.create('workspace-a', 'codex', 80, 24);
		await agentApi.restart('workspace-a', 'agent-a', 100, 30);
		await agentApi.stop('workspace-a', 'agent-a');
		await agentApi.delete('workspace-a', 'agent-a');

		expect(invoke.mock.calls).toEqual([
			['list_providers'],
			['refresh_providers'],
			['get_pi_project_trust', { workspaceId: 'workspace-a' }],
			['run_pi_rpc_probe', { workspaceId: 'workspace-a' }],
			['list_agent_sessions', { workspaceId: 'workspace-a' }],
			[
				'create_agent_session',
				{ workspaceId: 'workspace-a', providerId: 'codex', cols: 80, rows: 24 }
			],
			[
				'restart_agent_session',
				{ workspaceId: 'workspace-a', agentSessionId: 'agent-a', cols: 100, rows: 30 }
			],
			['stop_agent_session', { workspaceId: 'workspace-a', agentSessionId: 'agent-a' }],
			['delete_agent_session', { workspaceId: 'workspace-a', agentSessionId: 'agent-a' }]
		]);
	});
});

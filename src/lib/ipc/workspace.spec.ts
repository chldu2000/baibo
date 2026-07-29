import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { workspaceApi } from './workspace';

describe('workspaceApi', () => {
	beforeEach(() => {
		invoke.mockReset();
		invoke.mockResolvedValue({ workspaces: [], activeWorkspaceId: null });
	});

	it('uses the typed workspace command names and camel-case arguments', async () => {
		await workspaceApi.list();
		await workspaceApi.register();
		await workspaceApi.open('workspace-a');
		await workspaceApi.rename('workspace-a', 'Renamed');
		await workspaceApi.remove('workspace-a');

		expect(invoke.mock.calls).toEqual([
			['list_workspaces'],
			['register_workspace'],
			['open_workspace', { workspaceId: 'workspace-a' }],
			['rename_workspace', { workspaceId: 'workspace-a', name: 'Renamed' }],
			['remove_workspace', { workspaceId: 'workspace-a' }]
		]);
	});
});

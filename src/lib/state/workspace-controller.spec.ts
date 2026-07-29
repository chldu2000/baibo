import { describe, expect, it, vi } from 'vitest';

import type { WorkspaceRegistrySnapshot } from '$lib/domain/workspace';
import type { WorkspaceApi } from '$lib/ipc/workspace';

import { WorkspaceController } from './workspace.svelte';

const emptySnapshot: WorkspaceRegistrySnapshot = {
	workspaces: [],
	activeWorkspaceId: null
};

const populatedSnapshot: WorkspaceRegistrySnapshot = {
	workspaces: [
		{
			id: 'workspace-a',
			name: 'A',
			canonicalPath: '/tmp/a',
			repositoryRoot: '/tmp/a',
			gitRepository: true,
			createdAt: 1,
			lastOpenedAt: 2
		}
	],
	activeWorkspaceId: 'workspace-a'
};

function api(overrides: Partial<WorkspaceApi> = {}): WorkspaceApi {
	return {
		list: vi.fn().mockResolvedValue(emptySnapshot),
		register: vi.fn().mockResolvedValue(populatedSnapshot),
		open: vi.fn().mockResolvedValue(populatedSnapshot),
		rename: vi.fn().mockResolvedValue(populatedSnapshot),
		remove: vi.fn().mockResolvedValue(emptySnapshot),
		...overrides
	};
}

describe('WorkspaceController', () => {
	it('loads and applies snapshots from mutations', async () => {
		const controller = new WorkspaceController(api());

		await controller.load();
		expect(controller.snapshot).toEqual(emptySnapshot);
		await controller.register();
		expect(controller.snapshot).toEqual(populatedSnapshot);
		expect(controller.error).toBeNull();
	});

	it('treats a cancelled directory picker as a normal result', async () => {
		const controller = new WorkspaceController(api({ register: vi.fn().mockResolvedValue(null) }));

		const registered = await controller.register();

		expect(registered).toBe(false);
		expect(controller.error).toBeNull();
		expect(controller.snapshot).toEqual(emptySnapshot);
	});

	it('normalizes command errors and can clear them', async () => {
		const controller = new WorkspaceController(
			api({
				open: vi.fn().mockRejectedValue({ code: 'workspace_unavailable', message: '目录不可用' })
			})
		);

		const opened = await controller.open('workspace-a');

		expect(opened).toBe(false);
		expect(controller.error).toEqual({
			code: 'workspace_unavailable',
			message: '目录不可用'
		});
		controller.clearError();
		expect(controller.error).toBeNull();
	});

	it('serializes mutations while another request is pending', async () => {
		let finishList: (snapshot: WorkspaceRegistrySnapshot) => void = () => undefined;
		const list = vi.fn(
			() =>
				new Promise<WorkspaceRegistrySnapshot>((resolve) => {
					finishList = resolve;
				})
		);
		const register = vi.fn().mockResolvedValue(populatedSnapshot);
		const controller = new WorkspaceController(api({ list, register }));

		const loading = controller.load();
		const registered = await controller.register();
		finishList(emptySnapshot);
		await loading;

		expect(registered).toBe(false);
		expect(register).not.toHaveBeenCalled();
		expect(controller.pendingAction).toBeNull();
	});
});

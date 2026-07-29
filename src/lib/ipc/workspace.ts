import { invoke } from '@tauri-apps/api/core';

import type { WorkspaceRegistrySnapshot } from '$lib/domain/workspace';

export interface WorkspaceApi {
	list(): Promise<WorkspaceRegistrySnapshot>;
	register(): Promise<WorkspaceRegistrySnapshot | null>;
	open(workspaceId: string): Promise<WorkspaceRegistrySnapshot>;
	rename(workspaceId: string, name: string): Promise<WorkspaceRegistrySnapshot>;
	remove(workspaceId: string): Promise<WorkspaceRegistrySnapshot>;
}

export const workspaceApi: WorkspaceApi = {
	list: () => invoke<WorkspaceRegistrySnapshot>('list_workspaces'),
	register: () => invoke<WorkspaceRegistrySnapshot | null>('register_workspace'),
	open: (workspaceId) => invoke<WorkspaceRegistrySnapshot>('open_workspace', { workspaceId }),
	rename: (workspaceId, name) =>
		invoke<WorkspaceRegistrySnapshot>('rename_workspace', { workspaceId, name }),
	remove: (workspaceId) => invoke<WorkspaceRegistrySnapshot>('remove_workspace', { workspaceId })
};

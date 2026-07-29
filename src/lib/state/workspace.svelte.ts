import type { CommandError, WorkspaceRegistrySnapshot } from '$lib/domain/workspace';
import { emptyWorkspaceSnapshot } from '$lib/domain/workspace';
import { workspaceApi, type WorkspaceApi } from '$lib/ipc/workspace';

export type WorkspaceAction = 'loading' | 'registering' | 'opening' | 'renaming' | 'removing';

export class WorkspaceController {
	snapshot = $state.raw<WorkspaceRegistrySnapshot>(emptyWorkspaceSnapshot());
	pendingAction = $state<WorkspaceAction | null>(null);
	error = $state<CommandError | null>(null);

	#api: WorkspaceApi;

	constructor(api: WorkspaceApi = workspaceApi) {
		this.#api = api;
	}

	get loading(): boolean {
		return this.pendingAction === 'loading';
	}

	get busy(): boolean {
		return this.pendingAction !== null;
	}

	load = async (): Promise<void> => {
		await this.#run('loading', () => this.#api.list());
	};

	register = async (): Promise<boolean> => {
		const result = await this.#run('registering', () => this.#api.register());
		return result !== null && result !== undefined;
	};

	open = async (workspaceId: string): Promise<boolean> => {
		return (await this.#run('opening', () => this.#api.open(workspaceId))) !== undefined;
	};

	rename = async (workspaceId: string, name: string): Promise<boolean> => {
		return (await this.#run('renaming', () => this.#api.rename(workspaceId, name))) !== undefined;
	};

	remove = async (workspaceId: string): Promise<boolean> => {
		return (await this.#run('removing', () => this.#api.remove(workspaceId))) !== undefined;
	};

	clearError = (): void => {
		this.error = null;
	};

	async #run(
		action: WorkspaceAction,
		operation: () => Promise<WorkspaceRegistrySnapshot | null>
	): Promise<WorkspaceRegistrySnapshot | null | undefined> {
		if (this.busy) return undefined;

		this.pendingAction = action;
		this.error = null;
		try {
			const result = await operation();
			if (result) this.snapshot = result;
			return result;
		} catch (error) {
			this.error = normalizeCommandError(error);
			return undefined;
		} finally {
			this.pendingAction = null;
		}
	}
}

export function normalizeCommandError(error: unknown): CommandError {
	if (
		typeof error === 'object' &&
		error !== null &&
		'code' in error &&
		typeof error.code === 'string' &&
		'message' in error &&
		typeof error.message === 'string'
	) {
		return { code: error.code, message: error.message };
	}
	return {
		code: 'workspace_request_failed',
		message: error instanceof Error ? error.message : '工作空间操作失败，请重试。'
	};
}

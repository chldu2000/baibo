import { SvelteMap } from 'svelte/reactivity';

import type { AgentSession } from '$lib/domain/agent';
import type { ProviderId } from '$lib/domain/provider';
import type { CommandError } from '$lib/domain/workspace';
import { agentApi, type AgentApi } from '$lib/ipc/agent';
import { normalizeCommandError } from '$lib/state/workspace.svelte';

export type AgentAction = 'loading' | 'creating' | 'restarting' | 'stopping' | 'deleting';

export class AgentController {
	pendingAction = $state<AgentAction | null>(null);
	error = $state<CommandError | null>(null);
	#sessions = new SvelteMap<string, AgentSession[]>();
	#api: AgentApi;
	#operation: Promise<void> = Promise.resolve();

	constructor(api: AgentApi = agentApi) {
		this.#api = api;
	}

	get busy(): boolean {
		return this.pendingAction !== null;
	}

	sessions(workspaceId: string): AgentSession[] {
		return this.#sessions.get(workspaceId) ?? [];
	}

	byTerminal(workspaceId: string, terminalId: string): AgentSession | null {
		return this.sessions(workspaceId).find((session) => session.terminal.id === terminalId) ?? null;
	}

	load = async (workspaceId: string): Promise<void> => {
		await this.#mutate('loading', async () => {
			this.#sessions.set(workspaceId, await this.#api.list(workspaceId));
		});
	};

	create = async (
		workspaceId: string,
		providerId: ProviderId,
		cols: number,
		rows: number
	): Promise<AgentSession | null> => {
		let created: AgentSession | null = null;
		await this.#mutate('creating', async () => {
			created = await this.#api.create(workspaceId, providerId, cols, rows);
			this.#sessions.set(workspaceId, [created, ...this.sessions(workspaceId)]);
		});
		return created;
	};

	restart = async (
		workspaceId: string,
		agentSessionId: string,
		cols: number,
		rows: number
	): Promise<AgentSession | null> => {
		let restarted: AgentSession | null = null;
		await this.#mutate('restarting', async () => {
			restarted = await this.#api.restart(workspaceId, agentSessionId, cols, rows);
			this.#sessions.set(workspaceId, [restarted, ...this.sessions(workspaceId)]);
		});
		return restarted;
	};

	stop = async (workspaceId: string, agentSessionId: string): Promise<AgentSession | null> => {
		let stopped: AgentSession | null = null;
		await this.#mutate('stopping', async () => {
			stopped = await this.#api.stop(workspaceId, agentSessionId);
			this.#replace(stopped);
		});
		return stopped;
	};

	delete = async (workspaceId: string, agentSessionId: string): Promise<boolean> => {
		let deleted = false;
		await this.#mutate('deleting', async () => {
			await this.#api.delete(workspaceId, agentSessionId);
			this.#sessions.set(
				workspaceId,
				this.sessions(workspaceId).filter(({ id }) => id !== agentSessionId)
			);
			deleted = true;
		});
		return deleted;
	};

	clearError = (): void => {
		this.error = null;
	};

	#replace(session: AgentSession): void {
		this.#sessions.set(
			session.workspaceId,
			this.sessions(session.workspaceId).map((current) =>
				current.id === session.id ? session : current
			)
		);
	}

	async #mutate(action: AgentAction, operation: () => Promise<void>): Promise<void> {
		const previous = this.#operation;
		let release: () => void = () => undefined;
		this.#operation = new Promise<void>((resolve) => {
			release = resolve;
		});
		await previous;
		this.pendingAction = action;
		this.error = null;
		try {
			await operation();
		} catch (error) {
			this.error = normalizeCommandError(error);
		} finally {
			this.pendingAction = null;
			release();
		}
	}
}

import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import type { CommandError } from '$lib/domain/workspace';
import type { TerminalEvent, TerminalSession } from '$lib/domain/terminal';
import { terminalApi, type TerminalApi } from '$lib/ipc/terminal';
import { normalizeCommandError } from '$lib/state/workspace.svelte';

export type TerminalAction = 'loading' | 'creating' | 'stopping' | 'deleting';

interface WorkspaceTerminalState {
	sessions: TerminalSession[];
	activeTerminalId: string | null;
	closedTerminalIds: SvelteSet<string>;
}

export class TerminalController {
	pendingAction = $state<TerminalAction | null>(null);
	error = $state<CommandError | null>(null);
	#states = new SvelteMap<string, WorkspaceTerminalState>();
	#api: TerminalApi;
	#queuedLoadWorkspaceId: string | null = null;

	constructor(api: TerminalApi = terminalApi) {
		this.#api = api;
	}

	state(workspaceId: string): WorkspaceTerminalState {
		return (
			this.#states.get(workspaceId) ?? {
				sessions: [],
				activeTerminalId: null,
				closedTerminalIds: new SvelteSet()
			}
		);
	}

	get busy(): boolean {
		return this.pendingAction !== null;
	}

	load = async (workspaceId: string): Promise<void> => {
		if (this.busy) {
			this.#queuedLoadWorkspaceId = workspaceId;
			return;
		}
		this.pendingAction = 'loading';
		this.error = null;
		try {
			const sessions = await this.#api.list(workspaceId);
			const previous = this.state(workspaceId);
			this.#set(workspaceId, {
				...previous,
				sessions,
				activeTerminalId:
					previous.activeTerminalId && sessions.some(({ id }) => id === previous.activeTerminalId)
						? previous.activeTerminalId
						: (sessions[0]?.id ?? null)
			});
		} catch (error) {
			this.error = normalizeCommandError(error);
		} finally {
			this.pendingAction = null;
			this.#drainQueuedLoad();
		}
	};

	create = async (
		workspaceId: string,
		cols: number,
		rows: number
	): Promise<TerminalSession | null> => {
		if (this.busy) return null;
		this.pendingAction = 'creating';
		this.error = null;
		try {
			const session = await this.#api.create(workspaceId, cols, rows);
			const current = this.state(workspaceId);
			this.#set(workspaceId, {
				...current,
				sessions: [session, ...current.sessions],
				activeTerminalId: session.id,
				closedTerminalIds: without(current.closedTerminalIds, session.id)
			});
			return session;
		} catch (error) {
			this.error = normalizeCommandError(error);
			return null;
		} finally {
			this.pendingAction = null;
			this.#drainQueuedLoad();
		}
	};

	select = (workspaceId: string, terminalId: string): void => {
		const current = this.state(workspaceId);
		this.#set(workspaceId, {
			...current,
			activeTerminalId: terminalId,
			closedTerminalIds: without(current.closedTerminalIds, terminalId)
		});
	};

	closeView = (workspaceId: string, terminalId: string): void => {
		const current = this.state(workspaceId);
		const closedTerminalIds = new SvelteSet(current.closedTerminalIds).add(terminalId);
		const activeTerminalId =
			current.activeTerminalId === terminalId
				? (current.sessions.find(
						(session) => session.id !== terminalId && !closedTerminalIds.has(session.id)
					)?.id ?? null)
				: current.activeTerminalId;
		this.#set(workspaceId, { ...current, closedTerminalIds, activeTerminalId });
	};

	stop = async (workspaceId: string, terminalId: string): Promise<void> => {
		await this.#mutate('stopping', async () => {
			const session = await this.#api.stop(workspaceId, terminalId);
			this.updateSession(session);
		});
	};

	delete = async (workspaceId: string, terminalId: string): Promise<void> => {
		await this.#mutate('deleting', async () => {
			await this.#api.delete(workspaceId, terminalId);
			const current = this.state(workspaceId);
			const sessions = current.sessions.filter(({ id }) => id !== terminalId);
			this.#set(workspaceId, {
				sessions,
				activeTerminalId:
					current.activeTerminalId === terminalId
						? (sessions.find(({ id }) => !current.closedTerminalIds.has(id))?.id ?? null)
						: current.activeTerminalId,
				closedTerminalIds: without(current.closedTerminalIds, terminalId)
			});
		});
	};

	handleEvent = (event: TerminalEvent): void => {
		if (event.event === 'sessionUpdated') this.updateSession(event.data.session);
		if (event.event === 'outputLagged') {
			this.error = {
				code: 'terminal_output_lagged',
				message: '终端输出过快，部分持久化日志已截断；实时终端仍可继续使用。'
			};
		}
	};

	updateSession = (session: TerminalSession): void => {
		const current = this.state(session.workspaceId);
		const exists = current.sessions.some((item) => item.id === session.id);
		this.#set(session.workspaceId, {
			...current,
			sessions: exists
				? current.sessions.map((item) => (item.id === session.id ? session : item))
				: [session, ...current.sessions],
			activeTerminalId: exists ? current.activeTerminalId : session.id,
			closedTerminalIds: without(current.closedTerminalIds, session.id)
		});
	};

	forgetSession = (workspaceId: string, terminalId: string): void => {
		const current = this.state(workspaceId);
		const sessions = current.sessions.filter(({ id }) => id !== terminalId);
		this.#set(workspaceId, {
			sessions,
			activeTerminalId:
				current.activeTerminalId === terminalId
					? (sessions[0]?.id ?? null)
					: current.activeTerminalId,
			closedTerminalIds: without(current.closedTerminalIds, terminalId)
		});
	};

	clearError = (): void => {
		this.error = null;
	};

	reportError = (error: unknown): void => {
		this.error = normalizeCommandError(error);
	};

	#set(workspaceId: string, state: WorkspaceTerminalState): void {
		this.#states.set(workspaceId, state);
	}

	async #mutate(action: TerminalAction, operation: () => Promise<void>): Promise<void> {
		if (this.busy) return;
		this.pendingAction = action;
		this.error = null;
		try {
			await operation();
		} catch (error) {
			this.error = normalizeCommandError(error);
		} finally {
			this.pendingAction = null;
			this.#drainQueuedLoad();
		}
	}

	#drainQueuedLoad(): void {
		const workspaceId = this.#queuedLoadWorkspaceId;
		if (!workspaceId) return;
		this.#queuedLoadWorkspaceId = null;
		void this.load(workspaceId);
	}
}

function without(values: SvelteSet<string>, removed: string): SvelteSet<string> {
	const next = new SvelteSet(values);
	next.delete(removed);
	return next;
}

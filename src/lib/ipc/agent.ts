import { invoke } from '@tauri-apps/api/core';

import type { AgentSession } from '$lib/domain/agent';
import type { ProviderId } from '$lib/domain/provider';

export interface AgentApi {
	list(workspaceId: string): Promise<AgentSession[]>;
	create(
		workspaceId: string,
		providerId: ProviderId,
		cols: number,
		rows: number
	): Promise<AgentSession>;
	restart(
		workspaceId: string,
		agentSessionId: string,
		cols: number,
		rows: number
	): Promise<AgentSession>;
	stop(workspaceId: string, agentSessionId: string): Promise<AgentSession>;
	delete(workspaceId: string, agentSessionId: string): Promise<void>;
}

export const agentApi: AgentApi = {
	list: (workspaceId) => invoke('list_agent_sessions', { workspaceId }),
	create: (workspaceId, providerId, cols, rows) =>
		invoke('create_agent_session', { workspaceId, providerId, cols, rows }),
	restart: (workspaceId, agentSessionId, cols, rows) =>
		invoke('restart_agent_session', { workspaceId, agentSessionId, cols, rows }),
	stop: (workspaceId, agentSessionId) =>
		invoke('stop_agent_session', { workspaceId, agentSessionId }),
	delete: (workspaceId, agentSessionId) =>
		invoke('delete_agent_session', { workspaceId, agentSessionId })
};

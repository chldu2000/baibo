import type { ProviderId } from './provider';
import type { TerminalSession } from './terminal';

export interface AgentSession {
	id: string;
	workspaceId: string;
	providerId: ProviderId;
	providerSessionId: null;
	terminal: TerminalSession;
	launchMode: 'interactivePty';
	isolationMode: 'workspace';
	restartedFromSessionId: string | null;
	createdAt: number;
}

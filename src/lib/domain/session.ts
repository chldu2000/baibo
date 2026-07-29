import type { AgentSession } from './agent';
import type { SessionLifecycleEvent, TerminalLogIndex, TerminalSession } from './terminal';

export interface SessionDetail {
	terminal: TerminalSession;
	agentSession: AgentSession | null;
	lifecycleEvents: SessionLifecycleEvent[];
	logIndex: TerminalLogIndex;
}

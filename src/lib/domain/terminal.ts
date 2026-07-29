export type TerminalStatus =
	'starting' | 'running' | 'exited' | 'failed' | 'stopped' | 'interrupted';
export type SessionKind = 'shell' | 'agent' | 'legacy';

export interface TerminalSession {
	id: string;
	workspaceId: string;
	title: string;
	shell: string;
	cwd: string;
	status: TerminalStatus;
	cols: number;
	rows: number;
	createdAt: number;
	startedAt: number | null;
	endedAt: number | null;
	exitCode: number | null;
	terminationReason: string | null;
	sessionKind: SessionKind;
}

export type LifecycleEventKind =
	'created' | 'running' | 'exited' | 'failed' | 'stopped' | 'interrupted';

export interface SessionLifecycleEvent {
	terminalId: string;
	sequence: number;
	kind: LifecycleEventKind;
	status: TerminalStatus;
	occurredAt: number;
	exitCode: number | null;
	reason: string | null;
}

export type TerminalLogCoverage = 'complete' | 'truncated' | 'unknown';

export interface TerminalLogIndex {
	terminalId: string;
	firstSequence: number | null;
	lastSequence: number | null;
	chunkCount: number;
	retainedBytes: number;
	coverage: TerminalLogCoverage;
	updatedAt: number;
}

export interface TerminalAttachment {
	subscriptionId: string;
	session: TerminalSession;
}

export type TerminalEvent =
	| { event: 'sessionUpdated'; data: { session: TerminalSession } }
	| { event: 'outputLagged'; data: { terminalId: string } };

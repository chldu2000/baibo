export type TerminalStatus =
	'starting' | 'running' | 'exited' | 'failed' | 'stopped' | 'interrupted';

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
}

export interface TerminalAttachment {
	subscriptionId: string;
	session: TerminalSession;
}

export type TerminalEvent =
	| { event: 'sessionUpdated'; data: { session: TerminalSession } }
	| { event: 'outputLagged'; data: { terminalId: string } };

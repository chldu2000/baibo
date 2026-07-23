export type ProviderId = 'codex' | 'pi';

export interface AgentProvider {
	id: ProviderId;
	displayName: string;
	executable: string;
}

export const initialProviders: AgentProvider[] = [
	{ id: 'codex', displayName: 'Codex', executable: 'codex' },
	{ id: 'pi', displayName: 'Pi Agent', executable: 'pi' }
];

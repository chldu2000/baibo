export type ProviderId = 'codex' | 'pi';
export type ProviderAvailability =
	'checking' | 'available' | 'unavailable' | 'unsupported' | 'error';
export type CapabilitySupport = 'supported' | 'unsupported' | 'experimental';
export type ProviderLaunchMode = 'interactivePty' | 'rpc';

export interface ProviderCapabilities {
	interactivePty: CapabilitySupport;
	nativeResume: CapabilitySupport;
	structuredEvents: CapabilitySupport;
	approvals: CapabilitySupport;
	mcp: CapabilitySupport;
	rpc: CapabilitySupport;
	extensions: CapabilitySupport;
	skills: CapabilitySupport;
	projectTrust: CapabilitySupport;
}

export interface ProviderDiagnostic {
	code: string;
	message: string;
	recovery: string | null;
}

export interface ProviderInfo {
	id: ProviderId;
	displayName: string;
	availability: ProviderAvailability;
	executablePath: string | null;
	version: string | null;
	launchModes: ProviderLaunchMode[];
	capabilities: ProviderCapabilities;
	diagnostic: ProviderDiagnostic | null;
}

export interface PiProjectTrust {
	workspaceId: string;
	state: 'notRequired' | 'trusted' | 'denied' | 'promptRequired' | 'unknown';
	message: string;
}

export interface PiRpcProbeResult {
	providerId: 'pi';
	ok: boolean;
	message: string;
	elapsedMs: number;
}

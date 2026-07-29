import { invoke } from '@tauri-apps/api/core';

import type { PiProjectTrust, PiRpcProbeResult, ProviderInfo } from '$lib/domain/provider';

export interface ProviderApi {
	list(): Promise<ProviderInfo[]>;
	refresh(): Promise<ProviderInfo[]>;
	piTrust(workspaceId: string): Promise<PiProjectTrust>;
	runPiRpcProbe(workspaceId: string): Promise<PiRpcProbeResult>;
}

export const providerApi: ProviderApi = {
	list: () => invoke('list_providers'),
	refresh: () => invoke('refresh_providers'),
	piTrust: (workspaceId) => invoke('get_pi_project_trust', { workspaceId }),
	runPiRpcProbe: (workspaceId) => invoke('run_pi_rpc_probe', { workspaceId })
};

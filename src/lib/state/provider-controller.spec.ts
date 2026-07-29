import { describe, expect, it, vi } from 'vitest';

import type { PiProjectTrust, ProviderInfo } from '$lib/domain/provider';
import type { ProviderApi } from '$lib/ipc/provider';

import { ProviderController } from './provider.svelte';

const codex: ProviderInfo = {
	id: 'codex',
	displayName: 'Codex',
	availability: 'available',
	executablePath: '/usr/local/bin/codex',
	version: 'codex 1.0',
	launchModes: ['interactivePty'],
	capabilities: {
		interactivePty: 'supported',
		nativeResume: 'supported',
		structuredEvents: 'experimental',
		approvals: 'supported',
		mcp: 'supported',
		rpc: 'unsupported',
		extensions: 'unsupported',
		skills: 'supported',
		projectTrust: 'unsupported'
	},
	diagnostic: null
};

function api(overrides: Partial<ProviderApi> = {}): ProviderApi {
	return {
		list: vi.fn().mockResolvedValue([codex]),
		refresh: vi.fn().mockResolvedValue([codex]),
		piTrust: vi.fn().mockResolvedValue({
			workspaceId: 'workspace-a',
			state: 'promptRequired',
			message: 'Pi TUI 中确认'
		}),
		runPiRpcProbe: vi.fn(),
		...overrides
	};
}

describe('ProviderController', () => {
	it('loads, refreshes and exposes provider diagnostics', async () => {
		const controller = new ProviderController(api());
		await controller.load();
		await controller.refresh();

		expect(controller.provider('codex')).toEqual(codex);
		expect(controller.busy).toBe(false);
	});

	it('maps unreadable Pi trust to an explicit unknown state', async () => {
		const controller = new ProviderController(
			api({
				piTrust: vi
					.fn()
					.mockRejectedValue({ code: 'pi_trust_unknown', message: '无法读取 Pi trust' })
			})
		);

		await controller.loadPiTrust('workspace-a');

		expect(controller.piTrust?.state).toBe('unknown');
		expect(controller.error).toBeNull();
	});

	it('discards stale Pi trust responses from another workspace', async () => {
		let resolveA: ((value: Awaited<ReturnType<ProviderApi['piTrust']>>) => void) | undefined;
		let resolveB: ((value: Awaited<ReturnType<ProviderApi['piTrust']>>) => void) | undefined;
		const controller = new ProviderController(
			api({
				piTrust: vi.fn((workspaceId: string) => {
					return new Promise<PiProjectTrust>((resolve) => {
						if (workspaceId === 'workspace-a') resolveA = resolve;
						else resolveB = resolve;
					});
				})
			})
		);

		const requestA = controller.loadPiTrust('workspace-a');
		const requestB = controller.loadPiTrust('workspace-b');
		resolveB?.({
			workspaceId: 'workspace-b',
			state: 'trusted',
			message: 'B trusted'
		});
		await requestB;
		resolveA?.({
			workspaceId: 'workspace-a',
			state: 'denied',
			message: 'A denied'
		});
		await requestA;

		expect(controller.piTrust?.workspaceId).toBe('workspace-b');
		expect(controller.piTrustLoading).toBe(false);
	});

	it('invalidates an in-flight trust request when the dialog closes', async () => {
		let resolveTrust: ((value: Awaited<ReturnType<ProviderApi['piTrust']>>) => void) | undefined;
		const controller = new ProviderController(
			api({
				piTrust: vi.fn(
					() =>
						new Promise<PiProjectTrust>((resolve) => {
							resolveTrust = resolve;
						})
				)
			})
		);

		const request = controller.loadPiTrust('workspace-a');
		expect(controller.piTrustLoading).toBe(true);
		controller.clearPiTrust();
		resolveTrust?.({
			workspaceId: 'workspace-a',
			state: 'trusted',
			message: 'trusted'
		});
		await request;

		expect(controller.piTrust).toBeNull();
		expect(controller.piTrustLoading).toBe(false);
	});
});

import { describe, expect, it } from 'vitest';

import type { ProviderInfo } from './provider';

describe('ProviderInfo', () => {
	it('represents actionable availability without exposing environment values', () => {
		const provider: ProviderInfo = {
			id: 'pi',
			displayName: 'Pi',
			availability: 'unavailable',
			executablePath: null,
			version: null,
			launchModes: ['interactivePty', 'rpc'],
			capabilities: {
				interactivePty: 'supported',
				nativeResume: 'supported',
				structuredEvents: 'experimental',
				approvals: 'supported',
				mcp: 'unsupported',
				rpc: 'experimental',
				extensions: 'supported',
				skills: 'supported',
				projectTrust: 'supported'
			},
			diagnostic: {
				code: 'provider_executable_not_found',
				message: '登录环境中找不到 pi',
				recovery: '修正 PATH 后刷新'
			}
		};

		expect(provider).not.toHaveProperty('environment');
		expect(provider.diagnostic?.recovery).toContain('PATH');
	});
});

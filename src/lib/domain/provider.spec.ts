import { describe, expect, it } from 'vitest';

import { initialProviders } from './provider';

describe('initialProviders', () => {
	it('keeps Codex and Pi as the initial adapters', () => {
		expect(initialProviders.map(({ id }) => id)).toEqual(['codex', 'pi']);
	});
});

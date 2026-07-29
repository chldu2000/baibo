import { describe, expect, it, vi } from 'vitest';

import { TerminalResizeQueue } from './resize-queue';

describe('TerminalResizeQueue', () => {
	it('serializes resize calls and coalesces pending measurements to the latest size', async () => {
		let releaseFirst: () => void = () => undefined;
		const resize = vi
			.fn()
			.mockImplementationOnce(
				() =>
					new Promise<void>((resolve) => {
						releaseFirst = resolve;
					})
			)
			.mockResolvedValue(undefined);
		const queue = new TerminalResizeQueue(resize, vi.fn());

		queue.request(80, 24);
		queue.request(100, 30);
		queue.request(120, 40);
		expect(resize.mock.calls).toEqual([[80, 24]]);
		releaseFirst();
		await queue.idle();

		expect(resize.mock.calls).toEqual([
			[80, 24],
			[120, 40]
		]);
	});

	it('reports a resize error and continues with the latest pending size', async () => {
		const reportError = vi.fn();
		const resize = vi
			.fn()
			.mockRejectedValueOnce(new Error('resize failed'))
			.mockResolvedValue(undefined);
		const queue = new TerminalResizeQueue(resize, reportError);

		queue.request(80, 24);
		queue.request(90, 28);
		await queue.idle();

		expect(reportError).toHaveBeenCalledOnce();
		expect(resize.mock.calls.at(-1)).toEqual([90, 28]);
	});
});

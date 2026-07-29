import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	TERMINAL_INPUT_BATCH_BYTES,
	TERMINAL_INPUT_FLUSH_MS,
	TerminalInputBatcher
} from './input-batcher';

afterEach(() => {
	vi.useRealTimers();
});

describe('TerminalInputBatcher', () => {
	it('splits a large paste into ordered 4 KiB writes', async () => {
		const writes: Uint8Array[] = [];
		const batcher = new TerminalInputBatcher(async (data) => {
			writes.push(data);
		}, vi.fn());
		const input = Uint8Array.from({ length: 10_000 }, (_, index) => index % 251);

		batcher.push(input);
		batcher.flush();
		await batcher.idle();

		expect(writes.map(({ length }) => length)).toEqual([
			TERMINAL_INPUT_BATCH_BYTES,
			TERMINAL_INPUT_BATCH_BYTES,
			10_000 - TERMINAL_INPUT_BATCH_BYTES * 2
		]);
		expect(Uint8Array.from(writes.flatMap((write) => Array.from(write)))).toEqual(input);
	});

	it('flushes a partial batch after eight milliseconds', async () => {
		vi.useFakeTimers();
		const send = vi.fn().mockResolvedValue(undefined);
		const batcher = new TerminalInputBatcher(send, vi.fn());

		batcher.push(Uint8Array.of(1, 2, 3));
		await vi.advanceTimersByTimeAsync(TERMINAL_INPUT_FLUSH_MS - 1);
		expect(send).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		await batcher.idle();

		expect(send).toHaveBeenCalledWith(Uint8Array.of(1, 2, 3));
	});

	it('does not start a later write until the previous write completes', async () => {
		let releaseFirst: () => void = () => undefined;
		const send = vi
			.fn()
			.mockImplementationOnce(
				() =>
					new Promise<void>((resolve) => {
						releaseFirst = resolve;
					})
			)
			.mockResolvedValue(undefined);
		const batcher = new TerminalInputBatcher(send, vi.fn());

		batcher.push(new Uint8Array(TERMINAL_INPUT_BATCH_BYTES * 2));
		await Promise.resolve();
		expect(send).toHaveBeenCalledTimes(1);
		releaseFirst();
		await batcher.idle();

		expect(send).toHaveBeenCalledTimes(2);
	});
});

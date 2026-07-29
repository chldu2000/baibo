export const TERMINAL_INPUT_BATCH_BYTES = 4096;
export const TERMINAL_INPUT_FLUSH_MS = 8;

export class TerminalInputBatcher {
	#pending: number[] = [];
	#timer: ReturnType<typeof setTimeout> | null = null;
	#tail = Promise.resolve();

	constructor(
		private readonly send: (data: Uint8Array) => Promise<void>,
		private readonly reportError: (error: unknown) => void
	) {}

	push(data: Uint8Array): void {
		let offset = 0;
		while (offset < data.length) {
			const available = TERMINAL_INPUT_BATCH_BYTES - this.#pending.length;
			const length = Math.min(available, data.length - offset);
			this.#pending.push(...data.subarray(offset, offset + length));
			offset += length;
			if (this.#pending.length === TERMINAL_INPUT_BATCH_BYTES) this.#enqueuePending();
		}
		if (this.#pending.length > 0 && !this.#timer) {
			this.#timer = setTimeout(() => this.flush(), TERMINAL_INPUT_FLUSH_MS);
		}
	}

	flush(): void {
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = null;
		this.#enqueuePending();
	}

	dispose(): void {
		this.flush();
	}

	async idle(): Promise<void> {
		await this.#tail;
	}

	#enqueuePending(): void {
		if (this.#pending.length === 0) return;
		const batch = Uint8Array.from(this.#pending.splice(0));
		this.#tail = this.#tail.then(() => this.send(batch)).catch(this.reportError);
	}
}

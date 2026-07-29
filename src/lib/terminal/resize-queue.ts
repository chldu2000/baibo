interface TerminalSize {
	cols: number;
	rows: number;
}

export class TerminalResizeQueue {
	#pending: TerminalSize | null = null;
	#running = false;
	#disposed = false;
	#drainPromise: Promise<void> | null = null;

	constructor(
		private readonly resize: (cols: number, rows: number) => Promise<void>,
		private readonly reportError: (error: unknown) => void
	) {}

	request(cols: number, rows: number): void {
		if (this.#disposed) return;
		this.#pending = { cols, rows };
		if (!this.#running) {
			const drainPromise = this.#drain();
			this.#drainPromise = drainPromise;
			void drainPromise.finally(() => {
				if (this.#drainPromise === drainPromise) this.#drainPromise = null;
			});
		}
	}

	dispose(): void {
		this.#disposed = true;
		this.#pending = null;
	}

	async idle(): Promise<void> {
		await this.#drainPromise;
	}

	async #drain(): Promise<void> {
		this.#running = true;
		try {
			while (!this.#disposed && this.#pending) {
				const size = this.#pending;
				this.#pending = null;
				try {
					await this.resize(size.cols, size.rows);
				} catch (error) {
					this.reportError(error);
				}
			}
		} finally {
			this.#running = false;
		}
	}
}

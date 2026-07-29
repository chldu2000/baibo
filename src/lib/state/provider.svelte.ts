import type { CommandError } from '$lib/domain/workspace';
import type { PiProjectTrust, ProviderId, ProviderInfo } from '$lib/domain/provider';
import { providerApi, type ProviderApi } from '$lib/ipc/provider';
import { normalizeCommandError } from '$lib/state/workspace.svelte';

export class ProviderController {
	providers = $state<ProviderInfo[]>([]);
	loading = $state(false);
	refreshing = $state(false);
	piTrustLoading = $state(false);
	error = $state<CommandError | null>(null);
	piTrust = $state<PiProjectTrust | null>(null);
	#api: ProviderApi;
	#operation: Promise<void> = Promise.resolve();
	#piTrustRequest = 0;

	constructor(api: ProviderApi = providerApi) {
		this.#api = api;
	}

	get busy(): boolean {
		return this.loading || this.refreshing;
	}

	provider(id: ProviderId): ProviderInfo | null {
		return this.providers.find((provider) => provider.id === id) ?? null;
	}

	load = async (): Promise<void> => {
		await this.#serialize(async () => {
			this.loading = true;
			this.error = null;
			try {
				this.providers = await this.#api.list();
			} catch (error) {
				this.error = normalizeCommandError(error);
			} finally {
				this.loading = false;
			}
		});
	};

	refresh = async (): Promise<void> => {
		await this.#serialize(async () => {
			this.refreshing = true;
			this.error = null;
			try {
				this.providers = await this.#api.refresh();
			} catch (error) {
				this.error = normalizeCommandError(error);
			} finally {
				this.refreshing = false;
			}
		});
	};

	loadPiTrust = async (workspaceId: string): Promise<PiProjectTrust | null> => {
		const request = ++this.#piTrustRequest;
		this.piTrustLoading = true;
		this.piTrust = null;
		this.error = null;
		try {
			const trust = await this.#api.piTrust(workspaceId);
			if (request !== this.#piTrustRequest) return null;
			this.piTrust = trust;
			return trust;
		} catch (error) {
			if (request !== this.#piTrustRequest) return null;
			const normalized = normalizeCommandError(error);
			if (normalized.code === 'pi_trust_unknown') {
				this.piTrust = {
					workspaceId,
					state: 'unknown',
					message: normalized.message
				};
				return this.piTrust;
			}
			this.error = normalized;
			return null;
		} finally {
			if (request === this.#piTrustRequest) this.piTrustLoading = false;
		}
	};

	clearPiTrust = (): void => {
		this.#piTrustRequest += 1;
		this.piTrustLoading = false;
		this.piTrust = null;
	};

	clearError = (): void => {
		this.error = null;
	};

	async #serialize(operation: () => Promise<void>): Promise<void> {
		const previous = this.#operation;
		let release: () => void = () => undefined;
		this.#operation = new Promise<void>((resolve) => {
			release = resolve;
		});
		await previous;
		try {
			await operation();
		} finally {
			release();
		}
	}
}

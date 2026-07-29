import { createContext } from 'svelte';

import type { ProviderController } from './provider.svelte';

export const [getProviderController, setProviderController] = createContext<ProviderController>();

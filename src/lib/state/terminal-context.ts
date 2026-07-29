import { createContext } from 'svelte';

import type { TerminalController } from './terminal.svelte';

export const [getTerminalController, setTerminalController] = createContext<TerminalController>();

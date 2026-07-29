import { createContext } from 'svelte';

import type { AgentController } from './agent.svelte';

export const [getAgentController, setAgentController] = createContext<AgentController>();

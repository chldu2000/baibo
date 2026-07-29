import { createContext } from 'svelte';

import type { WorkspaceController } from './workspace.svelte';

export const [getWorkspaceController, setWorkspaceController] =
	createContext<WorkspaceController>();

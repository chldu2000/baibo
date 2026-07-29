export interface Workspace {
	id: string;
	name: string;
	canonicalPath: string;
	repositoryRoot: string | null;
	gitRepository: boolean;
	createdAt: number;
	lastOpenedAt: number;
}

export interface WorkspaceRegistrySnapshot {
	workspaces: Workspace[];
	activeWorkspaceId: string | null;
}

export interface CommandError {
	code: string;
	message: string;
}

export const emptyWorkspaceSnapshot = (): WorkspaceRegistrySnapshot => ({
	workspaces: [],
	activeWorkspaceId: null
});

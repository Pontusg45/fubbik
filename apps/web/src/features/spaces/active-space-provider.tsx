import { createContext, useCallback, useEffect, useState, type ReactNode } from "react";

const STORAGE_KEY_SPACE = "active-space";
const STORAGE_KEY_SPACE_LEGACY = "active-codebase"; // migrate old key
const STORAGE_KEY_WORKSPACE = "active-workspace";

export interface ActiveSpaceContextValue {
    spaceId: string | null;
    workspaceId: string | null;
    setSpaceId: (id: string | null) => void;
    setWorkspaceId: (id: string | null) => void;
}

export const ActiveSpaceContext = createContext<ActiveSpaceContextValue>({
    spaceId: null,
    workspaceId: null,
    setSpaceId: () => {},
    setWorkspaceId: () => {}
});

export function ActiveSpaceProvider({ children }: { children: ReactNode }) {
    const [spaceId, setSpaceIdRaw] = useState<string | null>(null);
    const [workspaceId, setWorkspaceIdRaw] = useState<string | null>(null);

    useEffect(() => {
        const savedWorkspace = localStorage.getItem(STORAGE_KEY_WORKSPACE);
        // Prefer new key, fall back to legacy "active-codebase" key, then migrate
        const savedSpace = localStorage.getItem(STORAGE_KEY_SPACE) ?? localStorage.getItem(STORAGE_KEY_SPACE_LEGACY);
        if (savedWorkspace) {
            setWorkspaceIdRaw(savedWorkspace);
        } else if (savedSpace) {
            setSpaceIdRaw(savedSpace);
            // Migrate: write to new key, remove old key
            localStorage.setItem(STORAGE_KEY_SPACE, savedSpace);
            localStorage.removeItem(STORAGE_KEY_SPACE_LEGACY);
        }
    }, []);

    const setSpaceId = useCallback((id: string | null) => {
        setSpaceIdRaw(id);
        setWorkspaceIdRaw(null);
        if (id) {
            localStorage.setItem(STORAGE_KEY_SPACE, id);
        } else {
            localStorage.removeItem(STORAGE_KEY_SPACE);
        }
        localStorage.removeItem(STORAGE_KEY_WORKSPACE);
        localStorage.removeItem(STORAGE_KEY_SPACE_LEGACY);
    }, []);

    const setWorkspaceId = useCallback((id: string | null) => {
        setWorkspaceIdRaw(id);
        setSpaceIdRaw(null);
        if (id) {
            localStorage.setItem(STORAGE_KEY_WORKSPACE, id);
        } else {
            localStorage.removeItem(STORAGE_KEY_WORKSPACE);
        }
        localStorage.removeItem(STORAGE_KEY_SPACE);
        localStorage.removeItem(STORAGE_KEY_SPACE_LEGACY);
    }, []);

    return (
        <ActiveSpaceContext.Provider value={{ spaceId, workspaceId, setSpaceId, setWorkspaceId }}>{children}</ActiveSpaceContext.Provider>
    );
}

import { createContext, useCallback, useState, type ReactNode } from "react";

const STORAGE_KEY_CODEBASE = "active-codebase";
const STORAGE_KEY_WORKSPACE = "active-workspace";

export interface ActiveCodebaseContextValue {
    codebaseId: string | null;
    workspaceId: string | null;
    setCodebaseId: (id: string | null) => void;
    setWorkspaceId: (id: string | null) => void;
}

export const ActiveCodebaseContext = createContext<ActiveCodebaseContextValue>({
    codebaseId: null,
    workspaceId: null,
    setCodebaseId: () => {},
    setWorkspaceId: () => {},
});

export function ActiveCodebaseProvider({ children }: { children: ReactNode }) {
    const [codebaseId, setCodebaseIdRaw] = useState<string | null>(() => {
        if (typeof localStorage !== "undefined") {
            return localStorage.getItem(STORAGE_KEY_CODEBASE);
        }
        return null;
    });

    const [workspaceId, setWorkspaceIdRaw] = useState<string | null>(() => {
        if (typeof localStorage !== "undefined") {
            return localStorage.getItem(STORAGE_KEY_WORKSPACE);
        }
        return null;
    });

    const setCodebaseId = useCallback((id: string | null) => {
        setCodebaseIdRaw(id);
        setWorkspaceIdRaw(null);
        if (id) {
            localStorage.setItem(STORAGE_KEY_CODEBASE, id);
        } else {
            localStorage.removeItem(STORAGE_KEY_CODEBASE);
        }
        localStorage.removeItem(STORAGE_KEY_WORKSPACE);
    }, []);

    const setWorkspaceId = useCallback((id: string | null) => {
        setWorkspaceIdRaw(id);
        setCodebaseIdRaw(null);
        if (id) {
            localStorage.setItem(STORAGE_KEY_WORKSPACE, id);
        } else {
            localStorage.removeItem(STORAGE_KEY_WORKSPACE);
        }
        localStorage.removeItem(STORAGE_KEY_CODEBASE);
    }, []);

    return (
        <ActiveCodebaseContext.Provider value={{ codebaseId, workspaceId, setCodebaseId, setWorkspaceId }}>
            {children}
        </ActiveCodebaseContext.Provider>
    );
}

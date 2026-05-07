import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";

const STORAGE_KEY_CODEBASE = "active-codebase";
const STORAGE_KEY_WORKSPACE = "active-workspace";

export function useActiveCodebase() {
    const search = useSearch({ strict: false }) as { codebase?: string; workspace?: string };
    const navigate = useNavigate();

    const codebaseId = search.codebase ?? null;
    const workspaceId = search.workspace ?? null;

    // Restore from localStorage when URL has no codebase/workspace params
    useEffect(() => {
        if (codebaseId || workspaceId) return;
        const savedWorkspace = localStorage.getItem(STORAGE_KEY_WORKSPACE);
        const savedCodebase = localStorage.getItem(STORAGE_KEY_CODEBASE);
        if (savedWorkspace) {
            void navigate({
                search: (prev: any) => ({ ...prev, workspace: savedWorkspace }),
                replace: true,
            } as any);
        } else if (savedCodebase) {
            void navigate({
                search: (prev: any) => ({ ...prev, codebase: savedCodebase }),
                replace: true,
            } as any);
        }
    }, []);

    // Persist to localStorage when selection changes
    useEffect(() => {
        if (workspaceId) {
            localStorage.setItem(STORAGE_KEY_WORKSPACE, workspaceId);
            localStorage.removeItem(STORAGE_KEY_CODEBASE);
        } else if (codebaseId) {
            localStorage.setItem(STORAGE_KEY_CODEBASE, codebaseId);
            localStorage.removeItem(STORAGE_KEY_WORKSPACE);
        }
    }, [codebaseId, workspaceId]);

    const setCodebaseId = useCallback(
        (id: string | null) => {
            void navigate({
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                search: (prev: any) => {
                    const { codebase: _, workspace: _w, ...rest } = prev as {
                        codebase?: string;
                        workspace?: string;
                    } & Record<string, unknown>;
                    if (id === null) {
                        localStorage.removeItem(STORAGE_KEY_CODEBASE);
                        return rest;
                    }
                    return { ...rest, codebase: id };
                }
            } as any);
        },
        [navigate]
    );

    const setWorkspaceId = useCallback(
        (id: string | null) => {
            void navigate({
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                search: (prev: any) => {
                    const { codebase: _, workspace: _w, ...rest } = prev as {
                        codebase?: string;
                        workspace?: string;
                    } & Record<string, unknown>;
                    if (id === null) {
                        localStorage.removeItem(STORAGE_KEY_WORKSPACE);
                        return rest;
                    }
                    return { ...rest, workspace: id };
                }
            } as any);
        },
        [navigate]
    );

    return { codebaseId, workspaceId, setCodebaseId, setWorkspaceId };
}

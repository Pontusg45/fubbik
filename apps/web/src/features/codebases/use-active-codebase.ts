import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback } from "react";

export function useActiveCodebase() {
    const search = useSearch({ strict: false }) as { codebase?: string; workspace?: string };
    const navigate = useNavigate();

    const codebaseId = search.codebase ?? null;
    const workspaceId = search.workspace ?? null;

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

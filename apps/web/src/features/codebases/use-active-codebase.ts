import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback } from "react";

export function useActiveCodebase() {
    const search = useSearch({ strict: false }) as { codebase?: string };
    const navigate = useNavigate();

    const codebaseId = search.codebase ?? null;

    const setCodebaseId = useCallback(
        (id: string | null) => {
            void navigate({
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                search: (prev: any) => {
                    if (id === null) {
                        const { codebase: _, ...rest } = prev as { codebase?: string } & Record<string, unknown>;
                        return rest;
                    }
                    return { ...prev, codebase: id };
                }
            } as any);
        },
        [navigate]
    );

    return { codebaseId, setCodebaseId };
}

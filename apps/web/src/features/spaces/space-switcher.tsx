import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { useActiveSpace } from "./use-active-space";

export function SpaceSwitcher() {
    const { spaceId, workspaceId, setSpaceId, setWorkspaceId } = useActiveSpace();

    const { data: spaces } = useQuery({
        queryKey: ["spaces"],
        queryFn: async () => unwrapEden(await api.api.spaces.get()),
        staleTime: 60_000 // spaces rarely change
    });

    const { data: workspaces } = useQuery({
        queryKey: ["workspaces"],
        queryFn: async () => unwrapEden(await api.api.workspaces.get()),
        staleTime: 60_000 // workspaces rarely change
    });

    // Auto-select the first space if none is active
    useEffect(() => {
        if (!spaceId && !workspaceId && spaces && spaces.length > 0) {
            setSpaceId(spaces[0]!.id);
        }
    }, [spaceId, workspaceId, spaces, setSpaceId]);

    const activeName = workspaceId
        ? (workspaces?.find((w: { id: string }) => w.id === workspaceId)?.name ?? "...")
        : spaceId
          ? (spaces?.find((c: { id: string }) => c.id === spaceId)?.name ?? "...")
          : "Select space";

    const hasWorkspaces = workspaces && workspaces.length > 0;

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" className="max-w-[180px] truncate" />}>
                {workspaceId ? `\u{1F4C2} ${activeName}` : activeName}
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
                {hasWorkspaces && (
                    <>
                        <DropdownMenuLabel>Workspaces</DropdownMenuLabel>
                        {workspaces.map((w: { id: string; name: string }) => (
                            <DropdownMenuItem
                                key={`ws-${w.id}`}
                                onClick={() => setWorkspaceId(w.id)}
                                className={workspaceId === w.id ? "bg-accent" : ""}
                            >
                                {w.name}
                            </DropdownMenuItem>
                        ))}
                        <DropdownMenuSeparator />
                        <DropdownMenuLabel>Spaces</DropdownMenuLabel>
                    </>
                )}
                {spaces?.map((c: { id: string; name: string }) => (
                    <DropdownMenuItem
                        key={c.id}
                        onClick={() => setSpaceId(c.id)}
                        className={spaceId === c.id && !workspaceId ? "bg-accent" : ""}
                    >
                        {c.name}
                    </DropdownMenuItem>
                ))}
                {(!spaces || spaces.length === 0) && !hasWorkspaces && <DropdownMenuItem disabled>No spaces registered</DropdownMenuItem>}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}

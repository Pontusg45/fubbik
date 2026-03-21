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
import { useActiveCodebase } from "./use-active-codebase";

export function CodebaseSwitcher() {
    const { codebaseId, workspaceId, setCodebaseId, setWorkspaceId } = useActiveCodebase();

    const { data: codebases } = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => unwrapEden(await api.api.codebases.get())
    });

    const { data: workspaces } = useQuery({
        queryKey: ["workspaces"],
        queryFn: async () => unwrapEden(await api.api.workspaces.get())
    });

    // Auto-select the first codebase if none is active
    useEffect(() => {
        if (!codebaseId && !workspaceId && codebases && codebases.length > 0) {
            setCodebaseId(codebases[0]!.id);
        }
    }, [codebaseId, workspaceId, codebases, setCodebaseId]);

    const activeName = workspaceId
        ? workspaces?.find((w: { id: string }) => w.id === workspaceId)?.name ?? "..."
        : codebaseId
          ? codebases?.find((c: { id: string }) => c.id === codebaseId)?.name ?? "..."
          : "Select codebase";

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
                        <DropdownMenuLabel>Codebases</DropdownMenuLabel>
                    </>
                )}
                {codebases?.map((c: { id: string; name: string }) => (
                    <DropdownMenuItem
                        key={c.id}
                        onClick={() => setCodebaseId(c.id)}
                        className={codebaseId === c.id && !workspaceId ? "bg-accent" : ""}
                    >
                        {c.name}
                    </DropdownMenuItem>
                ))}
                {(!codebases || codebases.length === 0) && !hasWorkspaces && (
                    <DropdownMenuItem disabled>No codebases registered</DropdownMenuItem>
                )}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}

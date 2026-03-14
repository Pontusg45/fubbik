import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";
import { useActiveCodebase } from "./use-active-codebase";

export function CodebaseSwitcher() {
    const { codebaseId, setCodebaseId } = useActiveCodebase();

    const { data: codebases } = useQuery({
        queryKey: ["codebases"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.codebases.get());
            } catch {
                return [];
            }
        }
    });

    // Auto-select the first codebase if none is active
    useEffect(() => {
        if (!codebaseId && codebases && codebases.length > 0) {
            setCodebaseId(codebases[0].id);
        }
    }, [codebaseId, codebases, setCodebaseId]);

    const activeName = codebaseId
        ? codebases?.find((c: { id: string }) => c.id === codebaseId)?.name ?? "..."
        : "Select codebase";

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" className="max-w-[180px] truncate" />}>
                {activeName}
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
                {codebases?.map((c: { id: string; name: string }) => (
                    <DropdownMenuItem
                        key={c.id}
                        onClick={() => setCodebaseId(c.id)}
                        className={codebaseId === c.id ? "bg-accent" : ""}
                    >
                        {c.name}
                    </DropdownMenuItem>
                ))}
                {(!codebases || codebases.length === 0) && (
                    <DropdownMenuItem disabled>No codebases registered</DropdownMenuItem>
                )}
            </DropdownMenuContent>
        </DropdownMenu>
    );
}

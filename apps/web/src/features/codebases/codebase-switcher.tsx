import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
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

    const activeName =
        codebaseId === "global"
            ? "Global"
            : codebaseId
              ? codebases?.find((c: { id: string }) => c.id === codebaseId)?.name ?? "..."
              : "All";

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" className="max-w-[150px] truncate" />}>
                {activeName}
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
                <DropdownMenuItem onClick={() => setCodebaseId(null)}>All</DropdownMenuItem>
                <DropdownMenuItem onClick={() => setCodebaseId("global")}>Global</DropdownMenuItem>
                <DropdownMenuSeparator />
                {codebases?.map((c: { id: string; name: string }) => (
                    <DropdownMenuItem key={c.id} onClick={() => setCodebaseId(c.id)}>
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

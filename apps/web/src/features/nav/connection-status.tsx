import { useQuery } from "@tanstack/react-query";
import { WifiOff } from "lucide-react";

import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function ConnectionStatus() {
    const { isError } = useQuery({
        queryKey: ["health"],
        queryFn: async () => unwrapEden(await legacyApi.api.health.get()),
        refetchInterval: 30_000,
        retry: false
    });

    if (!isError) return null;

    return (
        <div className="bg-destructive/10 text-destructive flex items-center justify-center gap-1.5 px-3 py-1.5 text-center text-xs">
            <WifiOff className="h-3 w-3" />
            Server unreachable — some features may be unavailable
        </div>
    );
}

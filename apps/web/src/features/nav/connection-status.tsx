import { useQuery } from "@tanstack/react-query";
import { WifiOff } from "lucide-react";
import { useEffect } from "react";

import { isNetworkError } from "@/lib/api-errors";
import { logApiOriginInDev } from "@/lib/api-origin";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function ConnectionStatus() {
    useEffect(() => {
        logApiOriginInDev();
    }, []);

    const health = useQuery({
        queryKey: ["health"],
        queryFn: async () => unwrapEden(await api.api.health.get()),
        refetchInterval: 30_000,
        retry: false
    });

    if (!health.isError) return null;

    const network = isNetworkError(health.error);
    const message = network
        ? "API server unreachable — run `just dev` and ensure port 3000 is free"
        : "API error — some features may be unavailable";

    return (
        <div className="bg-destructive/10 text-destructive flex items-center justify-center gap-1.5 px-3 py-1.5 text-center text-xs">
            <WifiOff className="h-3 w-3" />
            {message}
        </div>
    );
}

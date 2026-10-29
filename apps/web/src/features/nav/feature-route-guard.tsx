import { Link, useRouterState } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { PageLoading } from "@/components/ui/page";

import { FEATURE_LABEL, featureForPath, useCapabilitiesStatus } from "./use-capabilities";

/** Block only when capabilities are known and the route's feature is off; `null` (unknown) never blocks. */
export function shouldBlockRoute(pathname: string, enabled: Set<string> | null): boolean {
    if (enabled === null) return false;
    const feature = featureForPath(pathname);
    return feature !== undefined && !enabled.has(feature);
}

export type RouteGuardState = "render" | "loading" | "blocked";

/** A gated path waits for capabilities while they are pending; core paths always render at once. */
export function routeGuardState(pathname: string, enabled: Set<string> | null, pending: boolean): RouteGuardState {
    if (featureForPath(pathname) === undefined) return "render";
    if (pending) return "loading";
    return shouldBlockRoute(pathname, enabled) ? "blocked" : "render";
}

export function FeatureRouteGuard({ children }: { children: ReactNode }) {
    const pathname = useRouterState({ select: s => s.location.pathname });
    const { enabled, pending } = useCapabilitiesStatus();
    const state = routeGuardState(pathname, enabled, pending);
    const feature = featureForPath(pathname);
    if (state === "loading") return <PageLoading />;
    if (state === "render" || feature === undefined) return <>{children}</>;
    return (
        <div className="flex min-h-svh flex-col items-center justify-center gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">This feature is turned off</h1>
            <p className="text-muted-foreground">"{FEATURE_LABEL[feature]}" is disabled in this server's fubbik.toml.</p>
            <Link to="/dashboard" className="text-primary underline">
                Back to the dashboard
            </Link>
        </div>
    );
}

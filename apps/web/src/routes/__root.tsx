import type { QueryClient } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { Link, Outlet, createRootRouteWithContext, useLocation } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";
import { Settings } from "lucide-react";

import { ErrorBoundary } from "@/components/error-boundary";
import FubbikLogo from "@/components/fubbik-logo";
import { SmartLinkProvider } from "@/components/smart-link-provider";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Toaster } from "@/components/ui/sonner";
import UserMenu from "@/features/auth/user-menu";
import { CommandPalette } from "@/features/command-palette/command-palette";
import { FeatureSwitcher } from "@/features/feature-flags/feature-switcher";
import { Breadcrumbs } from "@/features/nav/breadcrumbs";
import { ConnectionStatus } from "@/features/nav/connection-status";
import { HeaderSearchBar } from "@/features/nav/header-search-bar";
import { KeyboardShortcutsHelp, useGlobalShortcuts } from "@/features/nav/keyboard-shortcuts";
import { MobileNav } from "@/features/nav/mobile-nav";
import { MANAGE_NAV } from "@/features/nav/navigation-items";
import { NotificationBell } from "@/features/nav/notification-bell";
import { ReadingTrailSidebar } from "@/features/nav/reading-trail-sidebar";
import { RecentlyViewed } from "@/features/nav/recently-viewed";
import { filterNavItems, useCapabilities } from "@/features/nav/use-capabilities";
import { usePendingProposalCount } from "@/features/proposals/use-pending-proposal-count";
import { ActiveSpaceProvider } from "@/features/spaces/active-space-provider";
import { SpaceSwitcher } from "@/features/spaces/space-switcher";
import { useStaleCount } from "@/features/staleness/use-stale-count";
import { VocabularyPrimer } from "@/features/vocabularies/use-vocabularies";

export interface RouterAppContext {
    queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterAppContext>()({
    component: RootDocument
});

function RootDocument() {
    const { helpOpen, setHelpOpen } = useGlobalShortcuts();
    const location = useLocation();
    const isLanding = location.pathname === "/";
    const staleCount = useStaleCount();
    const proposalCount = usePendingProposalCount();
    const capabilities = useCapabilities();
    const show = (to: string) => filterNavItems([{ to }], capabilities).length > 0;
    return (
        <>
            <ThemeProvider>
                <ActiveSpaceProvider>
                    <SmartLinkProvider>
                        <VocabularyPrimer />
                        <a
                            href="#main-content"
                            className="focus:bg-background sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:px-4 focus:py-2 focus:text-sm focus:shadow-lg"
                        >
                            Skip to main content
                        </a>
                        <div className={isLanding ? "min-h-svh" : ""}>
                            {!isLanding && (
                                <header className="border-b">
                                    <div className="container mx-auto flex items-center justify-between px-4 py-3">
                                        <Link to="/" className="flex items-center gap-2">
                                            <FubbikLogo className="size-6" />
                                            <span className="font-bold">fubbik</span>
                                        </Link>
                                        <SpaceSwitcher />
                                        <FeatureSwitcher />
                                        <nav aria-label="Main navigation" className="hidden items-center gap-1 md:flex">
                                            <Link
                                                to="/dashboard"
                                                className="text-muted-foreground hover:text-foreground [&.active]:text-foreground relative rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
                                            >
                                                Dashboard
                                                {staleCount > 0 && (
                                                    <span className="absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full bg-amber-500 text-[10px] font-bold text-white">
                                                        {staleCount > 9 ? "9+" : staleCount}
                                                    </span>
                                                )}
                                            </Link>
                                            <Link
                                                to="/chunks"
                                                search={{}}
                                                className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
                                            >
                                                Chunks
                                            </Link>
                                            <Link
                                                to="/graph"
                                                className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
                                            >
                                                Graph
                                            </Link>
                                            {show("/plans") && (
                                                <Link
                                                    to="/plans"
                                                    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
                                                >
                                                    Plans
                                                </Link>
                                            )}
                                            <HeaderSearchBar />
                                            <DropdownMenu>
                                                <DropdownMenuTrigger className="text-muted-foreground hover:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors">
                                                    <span className="flex items-center gap-1">
                                                        Manage
                                                        <Settings className="size-3.5" />
                                                    </span>
                                                </DropdownMenuTrigger>
                                                <DropdownMenuContent align="start">
                                                    <div className="text-muted-foreground px-2 py-1 text-xs font-semibold">Navigate</div>
                                                    {filterNavItems(MANAGE_NAV, capabilities).map(item => (
                                                        <DropdownMenuItem key={item.to} render={<Link to={item.to} search={{} as any} />}>
                                                            <item.icon className="size-4" />
                                                            {item.label}
                                                            {item.to === "/review" && proposalCount > 0 && (
                                                                <span className="ml-auto flex size-4 items-center justify-center rounded-full bg-amber-500 text-[10px] font-bold text-white">
                                                                    {proposalCount > 9 ? "9+" : proposalCount}
                                                                </span>
                                                            )}
                                                        </DropdownMenuItem>
                                                    ))}
                                                </DropdownMenuContent>
                                            </DropdownMenu>
                                        </nav>
                                        <div className="flex items-center gap-2">
                                            <MobileNav />
                                            <RecentlyViewed />
                                            <NotificationBell />
                                            <ThemeToggle />
                                            <UserMenu />
                                        </div>
                                    </div>
                                </header>
                            )}
                            {!isLanding && <ConnectionStatus />}
                            {!isLanding && <Breadcrumbs />}
                            <main id="main-content">
                                <ErrorBoundary>
                                    <Outlet />
                                </ErrorBoundary>
                            </main>
                        </div>
                        <CommandPalette />
                        {!isLanding && <ReadingTrailSidebar />}
                        <KeyboardShortcutsHelp open={helpOpen} onOpenChange={setHelpOpen} />
                        <Toaster richColors />
                    </SmartLinkProvider>
                </ActiveSpaceProvider>
            </ThemeProvider>

            <TanStackRouterDevtools position="bottom-left" />
            <ReactQueryDevtools position="bottom" buttonPosition="bottom-right" />
        </>
    );
}

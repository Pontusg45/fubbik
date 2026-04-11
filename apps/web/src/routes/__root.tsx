import type { QueryClient } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { HeadContent, Link, Outlet, Scripts, createRootRouteWithContext, useLocation } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";

import { Settings, SlidersHorizontal, Tags, FileText, BookOpen, Languages, Folder, FileSearch, FolderUp, Layers, Compass, MessageSquare } from "lucide-react";

import { ErrorBoundary } from "@/components/error-boundary";
import FubbikLogo from "@/components/fubbik-logo";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Toaster } from "@/components/ui/sonner";
import UserMenu from "@/features/auth/user-menu";

import { Breadcrumbs } from "@/features/nav/breadcrumbs";
import { HeaderSearchBar } from "@/features/nav/header-search-bar";
import { ConnectionStatus } from "@/features/nav/connection-status";
import { useStaleCount } from "@/features/staleness/use-stale-count";
import { KeyboardShortcutsHelp, useGlobalShortcuts } from "@/features/nav/keyboard-shortcuts";
import { MobileNav } from "@/features/nav/mobile-nav";
import { CodebaseSwitcher } from "@/features/codebases/codebase-switcher";
import { CommandPalette } from "@/features/command-palette/command-palette";
import { NotificationBell } from "@/features/nav/notification-bell";
import { RecentlyViewed } from "@/features/nav/recently-viewed";
import { ReadingTrailSidebar } from "@/features/nav/reading-trail-sidebar";

import appCss from "../index.css?url";

export interface RouterAppContext {
    queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterAppContext>()({
    head: () => ({
        meta: [
            {
                charSet: "utf-8"
            },
            {
                name: "viewport",
                content: "width=device-width, initial-scale=1"
            },
            {
                title: "Fubbik"
            }
        ],
        links: [
            {
                rel: "stylesheet",
                href: appCss
            }
        ]
    }),

    component: RootDocument
});

function RootDocument() {
    const { helpOpen, setHelpOpen } = useGlobalShortcuts();
    const location = useLocation();
    const isLanding = location.pathname === "/";
    const staleCount = useStaleCount();
    return (
        <html lang="en" suppressHydrationWarning>
            <head>
                <HeadContent />
            </head>
            <body>
                <ThemeProvider>
                    <a
                        href="#main-content"
                        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-4 focus:py-2 focus:text-sm focus:shadow-lg"
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
                                    <CodebaseSwitcher />
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
                                        <Link
                                            to="/requirements"
                                            className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
                                        >
                                            Requirements
                                        </Link>
                                        <HeaderSearchBar />
                                        <DropdownMenu>
                                            <DropdownMenuTrigger className="text-muted-foreground hover:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors">
                                                <span className="flex items-center gap-1">
                                                    Manage
                                                    <Settings className="size-3.5" />
                                                </span>
                                            </DropdownMenuTrigger>
                                            <DropdownMenuContent align="start">
                                                <DropdownMenuItem render={<Link to="/features" />}>
                                                    <Compass className="size-4" />
                                                    Features
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/reviews" />}>
                                                    <MessageSquare className="size-4" />
                                                    Reviews
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/docs" search={{}} />}>
                                                    <FileText className="size-4" />
                                                    Docs
                                                </DropdownMenuItem>
                                                <DropdownMenuSeparator />
                                                <DropdownMenuItem render={<Link to="/tags" />}>
                                                    <Tags className="size-4" />
                                                    Tags
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/templates" />}>
                                                    <FileText className="size-4" />
                                                    Templates
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/vocabulary" />}>
                                                    <Languages className="size-4" />
                                                    Vocabulary
                                                </DropdownMenuItem>
                                                <DropdownMenuSeparator />
                                                <DropdownMenuItem render={<Link to="/context" />}>
                                                    <FileSearch className="size-4" />
                                                    Context
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/codebases" />}>
                                                    <Folder className="size-4" />
                                                    Codebases
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/workspaces" />}>
                                                    <Layers className="size-4" />
                                                    Workspaces
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/knowledge-health" />}>
                                                    <BookOpen className="size-4" />
                                                    Health
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/activity" />}>
                                                    <BookOpen className="size-4" />
                                                    Activity
                                                </DropdownMenuItem>
                                                <DropdownMenuItem render={<Link to="/import" />}>
                                                    <FolderUp className="size-4" />
                                                    Import Docs
                                                </DropdownMenuItem>
                                                <DropdownMenuSeparator />
                                                <DropdownMenuItem render={<Link to="/settings" />}>
                                                    <SlidersHorizontal className="size-4" />
                                                    Settings
                                                </DropdownMenuItem>
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
                </ThemeProvider>

                <TanStackRouterDevtools position="bottom-left" />
                <ReactQueryDevtools position="bottom" buttonPosition="bottom-right" />
                <Scripts />
            </body>
        </html>
    );
}

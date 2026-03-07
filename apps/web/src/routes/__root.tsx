import type { QueryClient } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { HeadContent, Link, Outlet, Scripts, createRootRouteWithContext } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";

import { ErrorBoundary } from "@/components/error-boundary";
import FubbikLogo from "@/components/fubbik-logo";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";
import { Toaster } from "@/components/ui/sonner";
import UserMenu from "@/features/auth/user-menu";
import { MobileNav } from "@/features/nav/mobile-nav";
import { ChunkSearch } from "@/features/search/chunk-search";

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
    return (
        <html lang="en" suppressHydrationWarning>
            <head>
                <HeadContent />
            </head>
            <body>
                <ThemeProvider>
                    <div className="grid min-h-svh grid-rows-[auto_1fr]">
                        <header className="border-b">
                            <div className="container mx-auto flex items-center justify-between px-4 py-3">
                                <Link to="/" className="flex items-center gap-2">
                                    <FubbikLogo className="size-6" />
                                    <span className="font-bold">fubbik</span>
                                </Link>
                                <nav className="hidden items-center gap-1 md:flex">
                                    <Link to="/dashboard" className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground [&.active]:text-foreground">Dashboard</Link>
                                    <Link to="/chunks" search={{ page: 1, type: undefined, q: undefined, sort: undefined, tags: undefined, size: undefined, after: undefined, enrichment: undefined, minConnections: undefined }} className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground [&.active]:text-foreground">Chunks</Link>
                                    <Link to="/graph" className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground [&.active]:text-foreground">Graph</Link>
                                    <Link to="/tags" className="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground [&.active]:text-foreground">Tags</Link>
                                </nav>
                                <div className="flex items-center gap-2">
                                    <MobileNav />
                                    <ChunkSearch />
                                    <ThemeToggle />
                                    <UserMenu />
                                </div>
                            </div>
                        </header>
                        <main>
                            <ErrorBoundary>
                                <Outlet />
                            </ErrorBoundary>
                        </main>
                    </div>
                    <Toaster richColors />
                </ThemeProvider>

                <TanStackRouterDevtools position="bottom-left" />
                <ReactQueryDevtools position="bottom" buttonPosition="bottom-right" />
                <Scripts />
            </body>
        </html>
    );
}

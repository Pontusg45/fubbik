import type { QueryClient } from "@tanstack/react-query";

import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import {
  HeadContent,
  Link,
  Outlet,
  Scripts,
  createRootRouteWithContext,
} from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/react-router-devtools";

import FubbikLogo from "@/components/fubbik-logo";
import UserMenu from "@/components/user-menu";
import { Toaster } from "@/components/ui/sonner";

import appCss from "../index.css?url";

export interface RouterAppContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterAppContext>()({
  head: () => ({
    meta: [
      {
        charSet: "utf-8",
      },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      {
        title: "Fubbik",
      },
    ],
    links: [
      {
        rel: "stylesheet",
        href: appCss,
      },
    ],
  }),

  component: RootDocument,
});

function RootDocument() {
  return (
    <html lang="en" className="dark">
      <head>
        <HeadContent />
      </head>
      <body>
        <div className="grid min-h-svh grid-rows-[auto_1fr]">
          <header className="border-b">
            <div className="container mx-auto flex items-center justify-between px-4 py-3">
              <Link to="/" className="flex items-center gap-2">
                <FubbikLogo className="size-6" />
                <span className="font-bold">fubbik</span>
              </Link>
              <UserMenu />
            </div>
          </header>
          <main>
            <Outlet />
          </main>
        </div>
        <Toaster richColors />

        <TanStackRouterDevtools position="bottom-left" />
        <ReactQueryDevtools position="bottom" buttonPosition="bottom-right" />
        <Scripts />
      </body>
    </html>
  );
}

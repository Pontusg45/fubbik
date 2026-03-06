import "./index.css";
import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter as createTanStackRouter } from "@tanstack/react-router";
import { toast } from "sonner";

import ErrorState from "./components/error-state";
import Loader from "./components/loader";
import NotFound from "./components/not-found";
import { routeTree } from "./routeTree.gen";

export const queryClient = new QueryClient({
    queryCache: new QueryCache({
        onError: (error, query) => {
            toast.error(error.message, {
                action: {
                    label: "retry",
                    onClick: query.invalidate
                }
            });
        }
    }),
    defaultOptions: { queries: { staleTime: 60 * 1000 } }
});

export const getRouter = () => {
    const router = createTanStackRouter({
        routeTree,
        scrollRestoration: true,
        defaultPreloadStaleTime: 0,
        context: { queryClient },
        defaultPendingComponent: () => <Loader />,
        defaultNotFoundComponent: () => <NotFound />,
        defaultErrorComponent: ErrorState,
        Wrap: ({ children }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    });
    return router;
};

declare module "@tanstack/react-router" {
    interface Register {
        router: ReturnType<typeof getRouter>;
    }
}

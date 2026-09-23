import { expect, type Page, type Request, type Response, type Route } from "@playwright/test";
import { reportStep } from "./reporting";

export interface ApiEndpoint {
    readonly method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
    readonly path: `/api/${string}`;
}
export interface ExpectedResponse extends ApiEndpoint {
    readonly status: number;
    readonly timeout?: number;
}

/** Owns response waits, request recording and temporary failures for one test. */
export class ApiTraffic {
    private readonly cleanup = new Set<() => void>();
    readonly origin: string;
    constructor(
        private readonly page: Page,
        origin: string
    ) {
        this.origin = new URL(origin).origin;
    }
    private matches(endpoint: ApiEndpoint, request: Request) {
        const url = new URL(request.url());
        return url.origin === this.origin && url.pathname === endpoint.path && request.method() === endpoint.method;
    }
    async perform(endpoint: ExpectedResponse, action: () => Promise<unknown>): Promise<Response> {
        return reportStep(`${endpoint.method} ${endpoint.path} → ${endpoint.status}`, this.page, async () => {
            let stop = () => {};
            const response = new Promise<Response>((resolve, reject) => {
                const listener = (received: Response) => {
                    if (this.matches(endpoint, received.request())) resolve(received);
                };
                const timer = setTimeout(
                    () => reject(new Error(`Timed out waiting for ${endpoint.method} ${endpoint.path}`)),
                    endpoint.timeout ?? 15_000
                );
                stop = () => {
                    clearTimeout(timer);
                    this.page.off("response", listener);
                };
                this.page.on("response", listener);
            });
            try {
                // Attach the listener before starting the action, including synchronous failures.
                const [received] = await Promise.all([response, Promise.resolve().then(action)]);
                expect(received.status(), `${endpoint.method} ${endpoint.path} response status`).toBe(endpoint.status);
                return received;
            } finally {
                stop();
            }
        });
    }
    record(endpoint: ApiEndpoint) {
        const requests: Request[] = [];
        const listener = (request: Request) => {
            if (this.matches(endpoint, request)) requests.push(request);
        };
        this.page.on("request", listener);
        const stop = () => {
            this.page.off("request", listener);
            this.cleanup.delete(stop);
        };
        this.cleanup.add(stop);
        return { requests: requests as readonly Request[], stop };
    }
    async withFailure<T>(endpoint: ApiEndpoint, action: () => Promise<T>): Promise<T> {
        const url = (url: URL) => url.origin === this.origin && url.pathname === endpoint.path;
        const handler = async (route: Route) => {
            if (this.matches(endpoint, route.request())) {
                await route.fulfill({ status: 500, json: { message: "Injected test failure" } });
            } else await route.fallback();
        };
        await this.page.route(url, handler);
        try {
            return await action();
        } finally {
            await this.page.unroute(url, handler);
        }
    }
    dispose() {
        for (const stop of this.cleanup) stop();
    }
}

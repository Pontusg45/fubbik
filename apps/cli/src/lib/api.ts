import { outputError } from "./output";
import { getServerUrl } from "./store";

export function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        outputError('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

export async function fetchApi(path: string, opts?: RequestInit): Promise<Response> {
    const serverUrl = requireServer();
    return fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: {
            "Content-Type": "application/json",
            ...opts?.headers,
        },
    });
}

export async function fetchApiJson<T = unknown>(path: string, opts?: RequestInit): Promise<T> {
    const res = await fetchApi(path, opts);
    if (!res.ok) {
        const body = await res.text().catch(() => "");
        throw new Error(`API ${res.status}: ${body}`);
    }
    return res.json() as Promise<T>;
}

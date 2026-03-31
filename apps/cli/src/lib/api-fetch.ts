import { outputError } from "./output";

export async function apiFetch(url: string, init?: RequestInit): Promise<Response> {
    const res = await fetch(url, init);
    if (res.status === 429) {
        const body = (await res.json().catch(() => ({}))) as { retryAfter?: number };
        const wait = body.retryAfter ? `${body.retryAfter}s` : "a moment";
        outputError(`Rate limited. Try again in ${wait}.`);
        process.exit(1);
    }
    return res;
}

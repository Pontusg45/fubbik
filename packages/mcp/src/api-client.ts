export function getServerUrl(): string {
    return process.env["FUBBIK_SERVER_URL"] ?? "http://localhost:3000";
}

export async function apiFetch(path: string, options?: RequestInit): Promise<unknown> {
    const url = `${getServerUrl()}/api${path}`;
    const res = await fetch(url, {
        ...options,
        headers: { "Content-Type": "application/json", ...options?.headers }
    });
    if (!res.ok) {
        const text = await res.text();
        throw new Error(`API ${res.status}: ${text}`);
    }
    return res.json();
}

export function truncate(text: string | null | undefined, maxLength: number): string {
    if (!text) return "";
    return text.length > maxLength ? text.slice(0, maxLength) + "..." : text;
}

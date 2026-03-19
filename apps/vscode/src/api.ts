export interface Chunk {
    id: string;
    content: string;
    title: string | null;
    source: string | null;
    tags: string[];
    codebaseId: string | null;
    createdAt: string;
    updatedAt: string;
}

export interface CreateChunkBody {
    content: string;
    title?: string;
    source?: string;
    tags?: string[];
    codebaseId?: string;
}

export interface DetectResult {
    id: string;
    name: string;
    remoteUrl: string | null;
    localPath: string | null;
}

export class FubbikApi {
    private baseUrl: string;

    constructor(serverUrl: string) {
        this.baseUrl = serverUrl.replace(/\/+$/, "");
    }

    async detectCodebase(params: {
        remoteUrl?: string;
        localPath?: string;
    }): Promise<DetectResult | null> {
        try {
            const query = new URLSearchParams();
            if (params.remoteUrl) query.set("remoteUrl", params.remoteUrl);
            else if (params.localPath) query.set("localPath", params.localPath);
            const response = await fetch(`${this.baseUrl}/api/codebases/detect?${query}`);

            if (!response.ok) {
                return null;
            }

            return (await response.json()) as DetectResult;
        } catch {
            return null;
        }
    }

    async getChunks(
        codebaseId?: string
    ): Promise<{ chunks: Chunk[]; total: number }> {
        const url = new URL(`${this.baseUrl}/api/chunks`);
        if (codebaseId) {
            url.searchParams.set("codebaseId", codebaseId);
        }

        const response = await fetch(url.toString());

        if (!response.ok) {
            throw new Error(
                `Failed to fetch chunks: ${response.status} ${response.statusText}`
            );
        }

        return (await response.json()) as { chunks: Chunk[]; total: number };
    }

    async getChunk(id: string): Promise<{ chunk: Chunk; connections: unknown[] }> {
        const response = await fetch(`${this.baseUrl}/api/chunks/${id}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch chunk: ${response.status} ${response.statusText}`);
        }
        return response.json() as Promise<{ chunk: Chunk; connections: unknown[] }>;
    }

    async getTags(): Promise<Array<{ id: string; name: string }>> {
        try {
            const res = await fetch(`${this.baseUrl}/api/tags`);
            if (!res.ok) return [];
            return res.json();
        } catch {
            return [];
        }
    }

    async createChunk(body: CreateChunkBody): Promise<Chunk> {
        const response = await fetch(`${this.baseUrl}/api/chunks`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });

        if (!response.ok) {
            throw new Error(
                `Failed to create chunk: ${response.status} ${response.statusText}`
            );
        }

        return (await response.json()) as Chunk;
    }
}

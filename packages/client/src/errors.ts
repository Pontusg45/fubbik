export class NetworkError extends Error {
    readonly tag = "NetworkError" as const;

    constructor(message = "Could not reach the API server", options?: ErrorOptions) {
        super(message, options);
        this.name = "NetworkError";
    }
}

export class ApiError extends Error {
    readonly tag = "ApiError" as const;
    readonly status: number;
    readonly body: unknown;

    constructor(status: number, body: unknown, message?: string) {
        super(message ?? apiErrorMessage(status, body));
        this.name = "ApiError";
        this.status = status;
        this.body = body;
    }
}

export class ApiResponseParseError extends Error {
    readonly tag = "ApiResponseParseError" as const;
    readonly status: number;

    constructor(status: number, options?: ErrorOptions) {
        super(`API returned invalid JSON (${status})`, options);
        this.name = "ApiResponseParseError";
        this.status = status;
    }
}

function apiErrorMessage(status: number, body: unknown): string {
    if (body && typeof body === "object" && "message" in body && typeof (body as { message: unknown }).message === "string") {
        return (body as { message: string }).message;
    }
    return `Request failed (${status})`;
}

export function isNetworkError(error: unknown): error is NetworkError {
    return error instanceof NetworkError;
}

export function asNetworkError(error: unknown): NetworkError {
    if (error instanceof NetworkError) return error;
    return new NetworkError(undefined, { cause: error });
}

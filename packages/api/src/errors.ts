import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
    resource: string;
}> {}

export class AuthError extends Data.TaggedError("AuthError")<{}> {}

export class AiError extends Data.TaggedError("AiError")<{
    cause?: unknown;
}> {}

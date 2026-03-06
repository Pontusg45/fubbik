import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
    resource: string;
}> {}

export class AuthError extends Data.TaggedError("AuthError")<{}> {}

import { inject } from "vitest";

declare module "vitest" {
    export interface ProvidedContext {
        databaseUrl: string;
    }
}

// Set the isolated connection before any repository or environment module is imported.
process.env.DATABASE_URL = inject("databaseUrl");

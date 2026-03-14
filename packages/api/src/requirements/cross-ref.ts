import { lookupChunksByFilePath } from "@fubbik/db/repository";
import { Effect } from "effect";
import type { RequirementStep } from "@fubbik/db/schema/requirement";

export interface CrossRefWarning {
    step: number;
    type: "file_not_found" | "chunk_not_found";
    reference: string;
}

const FILE_PATH_REGEX = /(?:^|\s)([\w./-]+\.\w{1,10})(?:\s|$|[,;:)])/g;

function extractFilePaths(text: string): string[] {
    const paths: string[] = [];
    let match: RegExpExecArray | null;
    const regex = new RegExp(FILE_PATH_REGEX.source, FILE_PATH_REGEX.flags);
    while ((match = regex.exec(text)) !== null) {
        const path = match[1];
        // Must contain at least one slash to look like a file path
        if (path.includes("/")) {
            paths.push(path);
        }
    }
    return [...new Set(paths)];
}

export function crossReferenceSteps(
    steps: RequirementStep[],
    userId: string
): Effect.Effect<CrossRefWarning[], never> {
    return Effect.tryPromise({
        try: async () => {
            const warnings: CrossRefWarning[] = [];

            for (let i = 0; i < steps.length; i++) {
                const paths = extractFilePaths(steps[i].text);
                for (const path of paths) {
                    const results = await Effect.runPromise(
                        lookupChunksByFilePath(path, userId).pipe(
                            Effect.catchAll(() => Effect.succeed([]))
                        )
                    );
                    if (results.length === 0) {
                        warnings.push({
                            step: i,
                            type: "file_not_found",
                            reference: path
                        });
                    }
                }
            }

            return warnings;
        },
        catch: () => [] as CrossRefWarning[]
    }).pipe(Effect.catchAll(() => Effect.succeed([] as CrossRefWarning[])));
}

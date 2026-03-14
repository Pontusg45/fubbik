import {
    createTemplate as createTemplateRepo,
    deleteTemplate as deleteTemplateRepo,
    getTemplateById,
    listTemplates as listTemplatesRepo,
    updateTemplate as updateTemplateRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function listTemplates(userId: string) {
    return listTemplatesRepo(userId);
}

export function createTemplate(
    userId: string,
    body: { name: string; description?: string | null; type: string; content: string }
) {
    const id = crypto.randomUUID();
    return createTemplateRepo({
        id,
        name: body.name,
        description: body.description,
        type: body.type,
        content: body.content,
        userId
    });
}

export function updateTemplate(
    id: string,
    userId: string,
    body: { name?: string; description?: string | null; type?: string; content?: string }
) {
    return getTemplateById(id).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Template" }))
        ),
        Effect.flatMap(found => {
            if (found.isBuiltIn) {
                return Effect.fail(new ValidationError({ message: "Cannot edit built-in templates" }));
            }
            return updateTemplateRepo(id, userId, body);
        }),
        Effect.flatMap(updated =>
            updated ? Effect.succeed(updated) : Effect.fail(new NotFoundError({ resource: "Template" }))
        )
    );
}

export function deleteTemplate(id: string, userId: string) {
    return getTemplateById(id).pipe(
        Effect.flatMap(found =>
            found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Template" }))
        ),
        Effect.flatMap(found => {
            if (found.isBuiltIn) {
                return Effect.fail(new ValidationError({ message: "Cannot delete built-in templates" }));
            }
            return deleteTemplateRepo(id, userId);
        }),
        Effect.flatMap(deleted =>
            deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Template" }))
        )
    );
}

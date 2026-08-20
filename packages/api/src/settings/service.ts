import {
    getAllCodebaseSettings as getAllCodebaseSettingsRepo,
    getAllInstanceSettings as getAllInstanceSettingsRepo,
    getAllUserSettings as getAllUserSettingsRepo,
    setCodebaseSetting as setCodebaseSettingRepo,
    setInstanceSetting as setInstanceSettingRepo,
    setUserSetting as setUserSettingRepo,
    spaceBelongsTo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function getAllUserSettings(userId: string) {
    return getAllUserSettingsRepo(userId).pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) {
                map[row.key] = row.value;
            }
            return map;
        })
    );
}

export function setUserSetting(userId: string, key: string, value: unknown) {
    return setUserSettingRepo(userId, key, value);
}

export function getAllCodebaseSettings(spaceId: string, userId: string) {
    return getAllCodebaseSettingsRepo(spaceId, userId).pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) {
                map[row.key] = row.value;
            }
            return map;
        })
    );
}

/**
 * SECURITY: refuses to write settings for a space the caller does not own.
 * The repo write itself has no join to gate on (it is an upsert keyed by
 * spaceId), so the ownership check is an explicit pre-check here — the one
 * place in this fix where the guard cannot live in the query.
 */
export function setCodebaseSetting(spaceId: string, userId: string, key: string, value: unknown) {
    return spaceBelongsTo(spaceId, userId).pipe(
        Effect.flatMap(owned => (owned ? Effect.succeed(owned) : Effect.fail(new NotFoundError({ resource: "Space" })))),
        Effect.flatMap(() => setCodebaseSettingRepo(spaceId, key, value))
    );
}

export function getAllInstanceSettings() {
    return getAllInstanceSettingsRepo().pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) {
                map[row.key] = row.value;
            }
            return map;
        })
    );
}

export function setInstanceSetting(key: string, value: unknown) {
    return setInstanceSettingRepo(key, value);
}

export function getFeatureFlags() {
    return getAllInstanceSettingsRepo().pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) {
                map[row.key] = row.value;
            }
            return {
                aiEnabled: (map.aiEnabled as boolean) ?? true,
                enrichmentEnabled: (map.enrichmentEnabled as boolean) ?? true,
                semanticSearchEnabled: (map.semanticSearchEnabled as boolean) ?? true,
                aiSuggestionsEnabled: (map.aiSuggestionsEnabled as boolean) ?? true,
                vocabularySuggestEnabled: (map.vocabularySuggestEnabled as boolean) ?? true
            };
        })
    );
}

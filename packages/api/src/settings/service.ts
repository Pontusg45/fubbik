import {
    getAllCodebaseSettings as getAllCodebaseSettingsRepo,
    getAllInstanceSettings as getAllInstanceSettingsRepo,
    getAllUserSettings as getAllUserSettingsRepo,
    setCodebaseSetting as setCodebaseSettingRepo,
    setInstanceSetting as setInstanceSettingRepo,
    setUserSetting as setUserSettingRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

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

export function getAllCodebaseSettings(codebaseId: string) {
    return getAllCodebaseSettingsRepo(codebaseId).pipe(
        Effect.map(rows => {
            const map: Record<string, unknown> = {};
            for (const row of rows) {
                map[row.key] = row.value;
            }
            return map;
        })
    );
}

export function setCodebaseSetting(codebaseId: string, key: string, value: unknown) {
    return setCodebaseSettingRepo(codebaseId, key, value);
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

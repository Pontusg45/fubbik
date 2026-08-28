import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { codebaseSettings, instanceSettings, userSettings } from "../schema/settings";
import { space } from "../schema/space";

// --- User Settings ---

export function getUserSetting(userId: string, key: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select()
            .from(userSettings)
            .where(and(eq(userSettings.userId, userId), eq(userSettings.key, key)))
            .limit(1);
        return row ?? null;
    });
}

export function setUserSetting(userId: string, key: string, value: unknown) {
    return dbEffect(async () => {
        const id = crypto.randomUUID();
        const [row] = await db
            .insert(userSettings)
            .values({ id, userId, key, value })
            .onConflictDoUpdate({
                target: [userSettings.userId, userSettings.key],
                set: { value, updatedAt: new Date() }
            })
            .returning();
        return row!;
    });
}

export function getAllUserSettings(userId: string) {
    return dbEffect(() => db.select().from(userSettings).where(eq(userSettings.userId, userId)));
}

// --- Codebase Settings ---

export function getCodebaseSetting(spaceId: string, key: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select()
            .from(codebaseSettings)
            .where(and(eq(codebaseSettings.spaceId, spaceId), eq(codebaseSettings.key, key)))
            .limit(1);
        return row ?? null;
    });
}

export function setCodebaseSetting(spaceId: string, key: string, value: unknown) {
    return dbEffect(async () => {
        const id = crypto.randomUUID();
        const [row] = await db
            .insert(codebaseSettings)
            .values({ id, spaceId, key, value })
            .onConflictDoUpdate({
                target: [codebaseSettings.spaceId, codebaseSettings.key],
                set: { value, updatedAt: new Date() }
            })
            .returning();
        return row!;
    });
}

/**
 * SECURITY: scoped through the space's owner. `codebase_settings` has no
 * user column, so authority comes from the space — without this join, any
 * authenticated caller could read (and, via setCodebaseSetting, write) the
 * settings of any space by id.
 */
export function getAllCodebaseSettings(spaceId: string, userId: string) {
    return dbEffect(() =>
        db
            .select({
                id: codebaseSettings.id,
                spaceId: codebaseSettings.spaceId,
                key: codebaseSettings.key,
                value: codebaseSettings.value
            })
            .from(codebaseSettings)
            .innerJoin(space, eq(space.id, codebaseSettings.spaceId))
            .where(and(eq(codebaseSettings.spaceId, spaceId), eq(space.userId, userId)))
    );
}

/** True only if `spaceId` names a space owned by `userId`. */
export function spaceBelongsTo(spaceId: string, userId: string) {
    return dbEffect(async () => {
        const [row] = await db
            .select({ id: space.id })
            .from(space)
            .where(and(eq(space.id, spaceId), eq(space.userId, userId)))
            .limit(1);
        return Boolean(row);
    });
}

// --- Instance Settings ---

export function getInstanceSetting(key: string) {
    return dbEffect(async () => {
        const [row] = await db.select().from(instanceSettings).where(eq(instanceSettings.key, key)).limit(1);
        return row ?? null;
    });
}

export function setInstanceSetting(key: string, value: unknown) {
    return dbEffect(async () => {
        const [row] = await db
            .insert(instanceSettings)
            .values({ key, value })
            .onConflictDoUpdate({
                target: instanceSettings.key,
                set: { value, updatedAt: new Date() }
            })
            .returning();
        return row!;
    });
}

export function getAllInstanceSettings() {
    return dbEffect(() => db.select().from(instanceSettings));
}

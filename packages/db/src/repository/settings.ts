import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { codebaseSettings, instanceSettings, userSettings } from "../schema/settings";

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
    return dbEffect(() =>
            db
                .select()
                .from(userSettings)
                .where(eq(userSettings.userId, userId)));
}

// --- Codebase Settings ---

export function getCodebaseSetting(codebaseId: string, key: string) {
    return dbEffect(async () => {
            const [row] = await db
                .select()
                .from(codebaseSettings)
                .where(and(eq(codebaseSettings.codebaseId, codebaseId), eq(codebaseSettings.key, key)))
                .limit(1);
            return row ?? null;
        });
}

export function setCodebaseSetting(codebaseId: string, key: string, value: unknown) {
    return dbEffect(async () => {
            const id = crypto.randomUUID();
            const [row] = await db
                .insert(codebaseSettings)
                .values({ id, codebaseId, key, value })
                .onConflictDoUpdate({
                    target: [codebaseSettings.codebaseId, codebaseSettings.key],
                    set: { value, updatedAt: new Date() }
                })
                .returning();
            return row!;
        });
}

export function getAllCodebaseSettings(codebaseId: string) {
    return dbEffect(() =>
            db
                .select()
                .from(codebaseSettings)
                .where(eq(codebaseSettings.codebaseId, codebaseId)));
}

// --- Instance Settings ---

export function getInstanceSetting(key: string) {
    return dbEffect(async () => {
            const [row] = await db
                .select()
                .from(instanceSettings)
                .where(eq(instanceSettings.key, key))
                .limit(1);
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

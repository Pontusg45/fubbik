import { index, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

export const userSettings = pgTable(
    "user_settings",
    {
        id: text("id").primaryKey(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        key: text("key").notNull(),
        value: jsonb("value").notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("user_settings_user_key_idx").on(table.userId, table.key),
        index("user_settings_userId_idx").on(table.userId)
    ]
);

export const codebaseSettings = pgTable(
    "codebase_settings",
    {
        id: text("id").primaryKey(),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" }),
        key: text("key").notNull(),
        value: jsonb("value").notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("codebase_settings_cb_key_idx").on(table.codebaseId, table.key),
        index("codebase_settings_codebaseId_idx").on(table.codebaseId)
    ]
);

export const instanceSettings = pgTable("instance_settings", {
    key: text("key").primaryKey(),
    value: jsonb("value").notNull(),
    updatedAt: timestamp("updated_at")
        .defaultNow()
        .$onUpdate(() => new Date())
        .notNull()
});

// --- Known settings type maps ---

export interface UserSettingsMap {
    theme: "light" | "dark" | "system";
    defaultView: "list" | "kanban";
    defaultSort: "newest" | "oldest" | "alpha" | "updated";
    notificationsEnabled: boolean;
    notificationPollInterval: number;
    commandPaletteHistory: string[];
}

export interface CodebaseSettingsMap {
    defaultChunkType: string;
    requireReviewForAi: boolean;
    autoEnrichOnCreate: boolean;
    defaultTags: string[];
    templateId: string | null;
}

export interface InstanceSettingsMap {
    aiEnabled: boolean;
    ollamaUrl: string;
    enrichmentEnabled: boolean;
    semanticSearchEnabled: boolean;
    aiSuggestionsEnabled: boolean;
    vocabularySuggestEnabled: boolean;
    registrationEnabled: boolean;
    maxChunksPerCodebase: number;
}

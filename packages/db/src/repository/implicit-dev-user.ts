import { eq } from "drizzle-orm";

import { db } from "../index";
import { user } from "../schema/auth";

export const IMPLICIT_DEV_USER_ID = "dev-user";

/** Idempotent: creates the Better Auth `user` row used by implicit dev session (FK targets). */
export async function ensureImplicitDevUserRow(): Promise<boolean> {
    const [existing] = await db.select().from(user).where(eq(user.id, IMPLICIT_DEV_USER_ID));
    if (existing) return false;

    await db
        .insert(user)
        .values({
            id: IMPLICIT_DEV_USER_ID,
            name: "Dev User",
            email: "dev@localhost",
            emailVerified: false
        })
        .onConflictDoNothing({ target: user.id });

    const [again] = await db.select().from(user).where(eq(user.id, IMPLICIT_DEV_USER_ID));
    return Boolean(again);
}

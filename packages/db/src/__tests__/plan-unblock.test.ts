import { eq } from "drizzle-orm";
import { Effect } from "effect";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { db } from "../index";
import * as planRepo from "../repository/plan";
import { user } from "../schema/auth";
import { plan, planTask, planTaskDependency } from "../schema/plan";

describe("unblockDependentsOf", () => {
    let testUserId: string;
    let testPlanId: string;

    beforeEach(async () => {
        const userId = crypto.randomUUID();
        await db.insert(user).values({
            id: userId,
            email: `test-${Date.now()}@example.com`,
            name: "Test",
            emailVerified: false,
            createdAt: new Date(),
            updatedAt: new Date(),
        });
        testUserId = userId;

        const [p] = await db.insert(plan).values({ title: "Test", userId }).returning();
        if (!p) throw new Error("plan insert failed");
        testPlanId = p.id;
    });

    afterEach(async () => {
        await db.delete(plan).where(eq(plan.id, testPlanId));
        await db.delete(user).where(eq(user.id, testUserId));
    });

    it("unblocks tasks whose dependency is marked done", async () => {
        const [t1] = await db.insert(planTask).values({ planId: testPlanId, title: "Dep" }).returning();
        const [t2] = await db
            .insert(planTask)
            .values({ planId: testPlanId, title: "Dependent", status: "blocked" })
            .returning();
        if (!t1 || !t2) throw new Error("task insert failed");

        await db.insert(planTaskDependency).values({ taskId: t2.id, dependsOnTaskId: t1.id });

        const unblocked = await Effect.runPromise(planRepo.unblockDependentsOf(t1.id));

        expect(unblocked).toContain(t2.id);

        const [refreshed] = await db.select().from(planTask).where(eq(planTask.id, t2.id));
        expect(refreshed?.status).toBe("pending");
    });

    it("does not touch tasks already in in_progress", async () => {
        const [t1] = await db.insert(planTask).values({ planId: testPlanId, title: "Dep" }).returning();
        const [t2] = await db
            .insert(planTask)
            .values({ planId: testPlanId, title: "Dependent", status: "in_progress" })
            .returning();
        if (!t1 || !t2) throw new Error("task insert failed");

        await db.insert(planTaskDependency).values({ taskId: t2.id, dependsOnTaskId: t1.id });

        await Effect.runPromise(planRepo.unblockDependentsOf(t1.id));

        const [refreshed] = await db.select().from(planTask).where(eq(planTask.id, t2.id));
        expect(refreshed?.status).toBe("in_progress");
    });
});

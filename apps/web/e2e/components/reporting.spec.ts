import { expect, reportStep, test } from "../support/test";

test("visual steps attach a highlighted action image only when enabled", async ({ page, ui }, testInfo) => {
    // Given a real component fixture and a named action target.
    await page.goto("/");
    const title = ui.within(page.getByRole("form", { name: "Chunk settings" })).input("Title");
    const originalStyle = await title.root.getAttribute("style");
    const before = testInfo.attachments.length;

    // When a named helper step fills the field.
    await reportStep("When the title changes", title.root, () => title.fill("Visual report"));

    // Then the action succeeds, the temporary highlight is gone, and attachment mode is respected.
    await title.expectValue("Visual report");
    expect(await title.root.getAttribute("style")).toBe(originalStyle);
    const attachments = testInfo.attachments.slice(before);
    if (process.env.E2E_STEP_SCREENSHOTS === "1") {
        expect(attachments).toHaveLength(1);
        expect(attachments[0]?.name).toBe("Action: When the title changes");
        expect(attachments[0]?.contentType).toBe("image/png");
        expect(attachments[0]?.body?.length).toBeGreaterThan(100);
    } else expect(attachments).toHaveLength(0);
});

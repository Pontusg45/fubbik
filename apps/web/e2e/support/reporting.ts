import { test, type Locator, type Page } from "@playwright/test";

export type StepTarget = Page | Locator;

function isLocator(target: StepTarget): target is Locator {
    return "page" in target && typeof target.page === "function";
}

/** A named report step with one optional, annotated screenshot. */
export async function reportStep<T>(title: string, target: StepTarget, action: () => Promise<T>): Promise<T> {
    return test.step(title, async () => {
        if (process.env.E2E_STEP_SCREENSHOTS !== "1") return action();

        const page = isLocator(target) ? target.page() : target;
        if (isLocator(target)) await attachScreenshot(page, title, target);
        try {
            return await action();
        } finally {
            if (!isLocator(target)) await attachScreenshot(page, title);
        }
    });
}

async function attachScreenshot(page: Page, title: string, target?: Locator) {
    if (page.isClosed()) return;
    let restore: (() => Promise<void>) | undefined;
    try {
        if (target && (await target.count()) === 1 && (await target.isVisible())) {
            await target.scrollIntoViewIfNeeded();
            // Apply the highlight only while taking the screenshot, then restore inline styles.
            const previous = await target.evaluate(element => {
                const html = element as HTMLElement;
                const style = html.getAttribute("style");
                html.style.setProperty("outline", "4px solid #f97316", "important");
                html.style.setProperty("outline-offset", "4px", "important");
                return style;
            });
            restore = async () => {
                await target.evaluate((element, style) => {
                    if (style === null) element.removeAttribute("style");
                    else element.setAttribute("style", style);
                }, previous);
            };
        }
        await test.info().attach(`Action: ${title}`, {
            body: await page.screenshot({ animations: "disabled" }),
            contentType: "image/png"
        });
    } catch (error) {
        // Evidence must not replace the test's action result. Keep the capture error in the report.
        await test.info().attach(`Screenshot unavailable: ${title}`, {
            body: Buffer.from(String(error)),
            contentType: "text/plain"
        });
    } finally {
        if (restore) await restore().catch(() => {});
    }
}

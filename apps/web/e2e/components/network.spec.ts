import type { Page } from "@playwright/test";

import { expect, test } from "../support/test";

const origin = "http://127.0.0.1:4178";
const endpoint = { method: "POST", path: "/api/helper" } as const;
test.use({ apiOrigin: origin });

test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.route("**/api/**", route =>
        route.fulfill({ status: 200, headers: { "Access-Control-Allow-Origin": "*" }, json: { ok: true } })
    );
});

function request(page: Page, path = endpoint.path as string, method = "POST", host = origin) {
    return page.evaluate(
        async ({ url, method }) => {
            const response = await fetch(url, { method });
            await response.text();
            return response.status;
        },
        { url: `${host}${path}`, method }
    );
}

test("response waits and recordings match origin, method and exact path", async ({ page, network }) => {
    // Given a request recorder for one API endpoint.
    const recording = network.record(endpoint);
    // When unrelated requests precede the matching response, including a query string.
    const response = await network.perform({ ...endpoint, status: 200 }, async () => {
        await request(page, "/api/helper-extra");
        await request(page, "/api/helper", "GET");
        await request(page, "/api/helper", "POST", "http://localhost:4178");
        await request(page, "/api/helper?include=tags");
    });
    // Then only the exact method and path are recorded and the query is allowed.
    expect(recording.requests).toHaveLength(1);
    expect(response.url()).toBe(`${origin}/api/helper?include=tags`);
    // When recording is stopped, subsequent requests are ignored.
    recording.stop();
    await request(page);
    // Then the recording is unchanged.
    expect(recording.requests).toHaveLength(1);
});

test("temporary failures preserve other methods and are removed after success", async ({ page, network }) => {
    // Given an endpoint whose normal response is successful.
    // When a failure is installed for POST only.
    await network.withFailure(endpoint, async () => {
        await network.perform({ ...endpoint, status: 500 }, () => request(page));
        // Then GET still reaches the original handler.
        expect(await request(page, endpoint.path, "GET")).toBe(200);
    });
    // When the failure scope ends, POST reaches the original handler again.
    const response = await network.perform({ ...endpoint, status: 200 }, () => request(page));
    // Then the original response body is restored.
    expect(await response.json()).toEqual({ ok: true });
});

test("failure routes are removed even when the scoped action throws", async ({ page, network }) => {
    // Given a temporary failure handler.
    // When the action fails before completing.
    await expect(
        network.withFailure(endpoint, async () => {
            throw new Error("Action failed");
        })
    ).rejects.toThrow("Action failed");
    // Then the original endpoint is restored.
    expect(await request(page)).toBe(200);
});

test("response waits recover after failed actions and timeouts", async ({ page, network }) => {
    // Given an endpoint with a normal successful response.
    // When an action fails before issuing a request.
    await expect(
        network.perform({ ...endpoint, status: 200 }, async () => {
            throw new Error("Action failed");
        })
    ).rejects.toThrow("Action failed");
    // Then a subsequent response can still be observed.
    await network.perform({ ...endpoint, status: 200 }, () => request(page));
    // When the expected request never occurs.
    await expect(network.perform({ ...endpoint, status: 200, timeout: 50 }, async () => {})).rejects.toThrow(
        "Timed out waiting for POST /api/helper"
    );
    // Then another wait can complete after the timeout.
    await network.perform({ ...endpoint, status: 200 }, () => request(page));
});

test("status mismatches fail clearly and disposing stops recorders", async ({ page, network }) => {
    // Given a request recorder and an endpoint returning 200.
    const recording = network.record(endpoint);
    // When a different response status is expected.
    await expect(network.perform({ ...endpoint, status: 201 }, () => request(page))).rejects.toThrow("POST /api/helper response status");
    // Then the request is recorded even though the status assertion fails.
    expect(recording.requests).toHaveLength(1);
    // When fixture cleanup runs.
    network.dispose();
    await request(page);
    // Then the recorder no longer observes requests.
    expect(recording.requests).toHaveLength(1);
});

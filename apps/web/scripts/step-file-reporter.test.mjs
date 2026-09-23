import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import StepFileReporter from "./step-file-reporter.mjs";

test("exports nested step images to standalone HTML and Markdown sidecars", async () => {
    const outputDir = await mkdtemp(path.join(os.tmpdir(), "fubbik-step-report-"));
    try {
        const image = { name: "Action: Save", contentType: "image/png", body: Buffer.from("fake-png") };
        const attachmentStep = { category: "test.attach", attachments: [image], steps: [] };
        const step = { category: "test.step", title: "When <Save> is clicked", duration: 12, attachments: [], steps: [attachmentStep] };
        const reporter = new StepFileReporter({ format: "both", outputDir });
        reporter.onTestEnd({ titlePath: () => ["", "chromium", "reporting.spec.ts", "saves <title>"] }, {
            status: "passed", retry: 0, duration: 20, steps: [step], attachments: [image]
        });
        await reporter.onEnd();

        const html = await readFile(path.join(outputDir, "step-report.html"), "utf8");
        const markdown = await readFile(path.join(outputDir, "step-report.md"), "utf8");
        const assets = await readdir(path.join(outputDir, "step-report-assets"));
        assert.match(html, /data:image\/png;base64,/);
        assert.match(html, /When &lt;Save&gt; is clicked/);
        assert.doesNotMatch(html, /<title> is clicked/);
        assert.match(markdown, /When \\<Save\\> is clicked/);
        assert.match(markdown, /step-report-assets\/[a-f0-9]{16}\.png/);
        assert.equal(assets.length, 1);
        assert.deepEqual(await readFile(path.join(outputDir, "step-report-assets", assets[0])), image.body);
    } finally {
        await rm(outputDir, { recursive: true, force: true });
    }
});

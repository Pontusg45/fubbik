import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const escapeHtml = value => String(value).replace(/[&<>"']/g, char => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"
})[char]);
const escapeMarkdown = value => String(value).replace(/[\\`*_[\]<>|]/g, "\\$&").replace(/\r?\n/g, " ");
const imageType = type => ["image/png", "image/jpeg", "image/webp"].includes(type);
const extension = type => ({ "image/png": "png", "image/jpeg": "jpg", "image/webp": "webp" })[type];

async function evidence(attachments) {
    return Promise.all(attachments.filter(item => imageType(item.contentType)).map(async item => ({
        name: item.name,
        type: item.contentType,
        data: item.body ?? await readFile(item.path)
    })));
}

function stepAttachments(step) {
    return [
        ...step.attachments,
        ...step.steps.filter(child => child.category !== "test.step").flatMap(stepAttachments)
    ];
}

async function stepsFor(steps) {
    const result = [];
    for (const step of steps) {
        // Playwright API calls and assertions remain in its detailed HTML report.
        if (step.category !== "test.step") continue;
        result.push({
            title: step.title,
            duration: step.duration,
            error: step.error?.message ?? "",
            images: await evidence(stepAttachments(step)),
            children: await stepsFor(step.steps)
        });
    }
    return result;
}

function imageName(testIndex, trail, imageIndex, type) {
    const digest = createHash("sha256").update(`${testIndex}:${trail.join(":")}:${imageIndex}`).digest("hex").slice(0, 16);
    return `${digest}.${extension(type)}`;
}

function renderHtmlSteps(steps) {
    if (!steps.length) return "";
    return `<ol class="steps">${steps.map(step => `<li>
      <div class="step-title">${step.error ? "❌" : "✓"} ${escapeHtml(step.title)} <small>${step.duration} ms</small></div>
      ${step.error ? `<pre>${escapeHtml(step.error)}</pre>` : ""}
      ${step.images.map(image => `<figure><img loading="lazy" src="data:${image.type};base64,${image.data.toString("base64")}" alt="${escapeHtml(image.name)}"><figcaption>${escapeHtml(image.name)}</figcaption></figure>`).join("")}
      ${renderHtmlSteps(step.children)}
    </li>`).join("")}</ol>`;
}

function renderHtml(tests, errors) {
    const counts = Object.groupBy(tests, test => test.status);
    return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fubbik step report</title><style>
body{font:16px/1.5 system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#17212f;background:#f7f9fb}
h1{margin-bottom:.2rem}p.summary{color:#526173}details.test{background:white;border:1px solid #d5dde5;border-radius:8px;margin:1rem 0;padding:1rem}
summary{cursor:pointer;font-weight:650}.steps{border-left:2px solid #d5dde5;margin-left:.5rem;padding-left:1.5rem}.steps li{margin:1rem 0}
.step-title{font-weight:600}small{font-weight:400;color:#526173;margin-left:.5rem}figure{margin:.6rem 0 1.2rem}
img{display:block;max-width:100%;height:auto;border:1px solid #c6d0da;border-radius:6px}figcaption{font-size:.85rem;color:#526173}
pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#f1f4f7;padding:.75rem;border-radius:4px}
</style></head><body><h1>Fubbik step report</h1>
<p class="summary">${tests.length} attempts · ${counts.passed?.length ?? 0} passed · ${counts.failed?.length ?? 0} failed · ${counts.skipped?.length ?? 0} skipped</p>
${tests.map(test => `<details class="test" ${test.status === "failed" ? "open" : ""}><summary>${test.status === "passed" ? "✅" : test.status === "skipped" ? "○" : "❌"} ${escapeHtml(test.title)}${test.retry ? ` (retry ${test.retry})` : ""}</summary>
<p>${escapeHtml(test.status)} · ${test.duration} ms</p>${test.error ? `<pre>${escapeHtml(test.error)}</pre>` : ""}
${renderHtmlSteps(test.steps)}${test.images.map(image => `<figure><img loading="lazy" src="data:${image.type};base64,${image.data.toString("base64")}" alt="${escapeHtml(image.name)}"><figcaption>${escapeHtml(image.name)}</figcaption></figure>`).join("")}
</details>`).join("\n")}${errors.length ? `<h2>Runner errors</h2>${errors.map(error => `<pre>${escapeHtml(error)}</pre>`).join("")}` : ""}
</body></html>\n`;
}

async function renderMarkdown(tests, errors, outputDir) {
    const assetDir = path.join(outputDir, "step-report-assets");
    const lines = ["# Fubbik step report", "", `${tests.length} test attempts`, ""];
    async function addImages(images, testIndex, trail, indent) {
        for (const [index, image] of images.entries()) {
            const name = imageName(testIndex, trail, index, image.type);
            await mkdir(assetDir, { recursive: true });
            await writeFile(path.join(assetDir, name), image.data);
            lines.push(`${indent}![${escapeMarkdown(image.name)}](step-report-assets/${name})`, "");
        }
    }
    async function addSteps(steps, testIndex, trail = [], depth = 0) {
        for (const [index, step] of steps.entries()) {
            const next = [...trail, index];
            const indent = "  ".repeat(depth);
            lines.push(`${indent}- ${step.error ? "❌" : "✓"} ${escapeMarkdown(step.title)} (${step.duration} ms)`);
            if (step.error) lines.push(`${indent}  - Error: ${escapeMarkdown(step.error)}`);
            await addImages(step.images, testIndex, next, `${indent}  `);
            await addSteps(step.children, testIndex, next, depth + 1);
        }
    }
    for (const [index, test] of tests.entries()) {
        lines.push(`## ${test.status === "passed" ? "✅" : test.status === "skipped" ? "○" : "❌"} ${escapeMarkdown(test.title)}`, "", `Status: ${test.status}; duration: ${test.duration} ms${test.retry ? `; retry: ${test.retry}` : ""}`, "");
        if (test.error) lines.push(`> ${escapeMarkdown(test.error)}`, "");
        await addSteps(test.steps, index);
        await addImages(test.images, index, ["test"], "");
        lines.push("");
    }
    if (errors.length) lines.push("## Runner errors", "", ...errors.map(error => `> ${escapeMarkdown(error)}`), "");
    return `${lines.join("\n")}\n`;
}

export default class StepFileReporter {
    constructor({ format, outputDir }) {
        this.format = format;
        this.outputDir = path.resolve(outputDir);
        this.tests = [];
        this.errors = [];
    }

    onTestEnd(test, result) {
        this.tests.push({ test, result });
    }

    onError(error) {
        this.errors.push(error.message ?? String(error));
    }

    async onEnd() {
        await mkdir(this.outputDir, { recursive: true });
        const tests = [];
        for (const { test, result } of this.tests) tests.push({
            title: test.titlePath().filter(Boolean).join(" › "),
            status: result.status,
            retry: result.retry,
            duration: result.duration,
            error: result.error?.message ?? "",
            steps: await stepsFor(result.steps),
            images: await evidence(result.attachments.filter(item => !item.name.startsWith("Action: ")))
        });
        if (this.format === "html" || this.format === "both") {
            const file = path.join(this.outputDir, "step-report.html");
            await writeFile(file, renderHtml(tests, this.errors));
            process.stdout.write(`Step report: ${file}\n`);
        }
        if (this.format === "md" || this.format === "both") {
            const markdown = await renderMarkdown(tests, this.errors, this.outputDir);
            const file = path.join(this.outputDir, "step-report.md");
            await writeFile(file, markdown);
            process.stdout.write(`Step report: ${file}\n`);
        }
    }
}

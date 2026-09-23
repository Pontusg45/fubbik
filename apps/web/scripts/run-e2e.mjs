import { spawn } from "node:child_process";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const args = process.argv.slice(2);
let screenshots = false;
let report = "";
let reportRequested = false;
const forwarded = [];
for (let index = 0; index < args.length; index++) {
    const arg = args[index];
    if (arg === "--step-screenshots") screenshots = true;
    else if (arg === "--no-step-screenshots") screenshots = false;
    else if (arg === "--no-step-report") { report = ""; reportRequested = false; }
    else if (arg === "--step-report") { report = args[++index] ?? ""; reportRequested = true; }
    else if (arg.startsWith("--step-report=")) { report = arg.slice("--step-report=".length); reportRequested = true; }
    else forwarded.push(arg);
}
if (reportRequested && !["html", "md", "both"].includes(report)) {
    console.error("--step-report must be html, md, or both");
    process.exit(2);
}

const child = spawn(process.execPath, [require.resolve("@playwright/test/cli"), "test", ...forwarded], {
    stdio: "inherit",
    env: { ...process.env, E2E_STEP_SCREENSHOTS: screenshots ? "1" : "0", E2E_STEP_REPORT: report }
});
child.on("error", error => {
    console.error(error);
    process.exitCode = 1;
});
child.on("exit", (code, signal) => {
    if (signal) process.kill(process.pid, signal);
    else process.exitCode = code ?? 1;
});

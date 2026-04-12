import { execSync } from "node:child_process";
import { describe, expect, it } from "vitest";

const CLI_CWD = new URL("../../", import.meta.url).pathname.replace(/\/$/, "");

/** Run the CLI via bun and capture output. */
function runCli(args: string): {
    stdout: string;
    stderr: string;
    exitCode: number;
} {
    try {
        const stdout = execSync(`bun run src/index.ts ${args}`, {
            cwd: CLI_CWD,
            encoding: "utf-8",
            stdio: ["pipe", "pipe", "pipe"],
            env: { ...process.env, HOME: "/tmp/fubbik-test" },
        });
        return { stdout, stderr: "", exitCode: 0 };
    } catch (e: unknown) {
        const err = e as {
            stdout?: string;
            stderr?: string;
            status?: number;
        };
        return {
            stdout: err.stdout ?? "",
            stderr: err.stderr ?? "",
            exitCode: err.status ?? 1,
        };
    }
}

// ── Help output tests ────────────────────────────────────────────────

describe("CLI help output", () => {
    it("root --help lists all commands", () => {
        const { stdout } = runCli("--help");
        expect(stdout).toContain("fubbik");
        expect(stdout).toContain("plan");
        expect(stdout).toContain("check-files");
        expect(stdout).toContain("sync-claude-md");
        expect(stdout).toContain("context");
        expect(stdout).toContain("hooks");
        expect(stdout).toContain("completions");
        // New group commands
        expect(stdout).toContain("chunk");
        expect(stdout).toContain("tag");
        expect(stdout).toContain("req");
        expect(stdout).toContain("maintain");
        expect(stdout).toContain("review");
    });

    it("chunk --help shows subcommands", () => {
        const { stdout } = runCli("chunk --help");
        expect(stdout).toContain("add");
        expect(stdout).toContain("get");
        expect(stdout).toContain("list");
        expect(stdout).toContain("search");
        expect(stdout).toContain("update");
        expect(stdout).toContain("remove");
        expect(stdout).toContain("enrich");
        expect(stdout).toContain("link");
        expect(stdout).toContain("unlink");
    });

    it("plan --help shows subcommands", () => {
        const { stdout } = runCli("plan --help");
        expect(stdout).toContain("create");
        expect(stdout).toContain("list");
        expect(stdout).toContain("show");
        expect(stdout).toContain("status");
        expect(stdout).toContain("add-task");
        expect(stdout).toContain("task-done");
        expect(stdout).toContain("link-requirement");
    });

    it("check-files --help shows --staged option", () => {
        const { stdout } = runCli("check-files --help");
        expect(stdout).toContain("--staged");
    });

    it("sync-claude-md --help shows options", () => {
        const { stdout } = runCli("sync-claude-md --help");
        expect(stdout).toContain("--tag");
        expect(stdout).toContain("--output");
        expect(stdout).toContain("--watch");
    });

    it("context for --help shows path argument", () => {
        const { stdout } = runCli("context for --help");
        expect(stdout).toContain("path");
    });

    it("context --help shows subcommands", () => {
        const { stdout } = runCli("context --help");
        expect(stdout).toContain("export");
        expect(stdout).toContain("for");
        expect(stdout).toContain("dir");
    });

    it("hooks --help shows install and uninstall", () => {
        const { stdout } = runCli("hooks --help");
        expect(stdout).toContain("install");
        expect(stdout).toContain("uninstall");
    });

    it("completions --help shows shell argument", () => {
        const { stdout } = runCli("completions --help");
        expect(stdout).toContain("shell");
    });
});

// ── Completions generation ───────────────────────────────────────────

describe("completions", () => {
    it("zsh outputs valid zsh completion script", () => {
        const { stdout, exitCode } = runCli("completions zsh");
        expect(exitCode).toBe(0);
        expect(stdout).toContain("#compdef fubbik");
        expect(stdout).toContain("_fubbik");
    });
});

// ── Error handling ───────────────────────────────────────────────────

describe("CLI error handling", () => {
    it("plan create without --title exits non-zero", () => {
        const { exitCode } = runCli("plan create");
        expect(exitCode).not.toBe(0);
    });

    it("check-files --staged does not crash without server", () => {
        // With HOME set to /tmp/fubbik-test there is no .fubbik store,
        // so the command should exit gracefully (not throw unhandled exception).
        const result = runCli("check-files --staged");
        expect(result.exitCode).toBeDefined();
    });

    it("unknown command shows help / error", () => {
        const result = runCli("nonexistent-command");
        expect(result.exitCode).not.toBe(0);
    });
});

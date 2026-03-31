import { execSync } from "node:child_process";
import { Command } from "commander";

const ROUTES: Record<string, string> = {
    dashboard: "/dashboard",
    graph: "/graph",
    chunks: "/chunks",
    requirements: "/requirements",
    plans: "/plans",
    import: "/import",
    settings: "/settings",
    health: "/knowledge-health",
    tags: "/tags",
    docs: "/docs",
};

function openUrl(url: string) {
    try {
        execSync(`open "${url}"`, { stdio: "ignore" });
    } catch {
        try {
            execSync(`xdg-open "${url}"`, { stdio: "ignore" });
        } catch {
            console.log(`Open: ${url}`);
        }
    }
}

export const openCommand = new Command("open")
    .description("Open fubbik in the browser")
    .argument("[target]", `page or chunk ID (${Object.keys(ROUTES).join(", ")}, <chunk-id>)`)
    .action((target?: string) => {
        const webUrl = "http://localhost:3001";
        if (!target) {
            openUrl(`${webUrl}/dashboard`);
            return;
        }
        if (ROUTES[target]) {
            openUrl(`${webUrl}${ROUTES[target]}`);
            return;
        }
        // Assume chunk ID
        openUrl(`${webUrl}/chunks/${target}`);
    });

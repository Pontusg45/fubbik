import { Command } from "commander";

export const mcpToolsCommand = new Command("mcp-tools")
    .description("List available MCP server tools")
    .action(() => {
        console.log("MCP tools command not yet implemented.");
    });

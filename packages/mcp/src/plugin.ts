import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";

export interface McpPlugin {
    name: string;
    description: string;
    register: (server: McpServer) => void;
}

const plugins: McpPlugin[] = [];

export function registerPlugin(plugin: McpPlugin) {
    plugins.push(plugin);
}

export function loadAllPlugins(server: McpServer) {
    for (const plugin of plugins) {
        plugin.register(server);
        console.log(`[mcp] Loaded plugin: ${plugin.name} (${plugin.description})`);
    }
}

export function listPlugins(): Array<{ name: string; description: string }> {
    return plugins.map(p => ({ name: p.name, description: p.description }));
}

#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

import { registerPlugin, loadAllPlugins } from "./plugin.js";
import { corePlugin } from "./tools.js";
import { sessionPlugin } from "./session-tools.js";
import { suggestionPlugin } from "./suggestion-tools.js";
import { planPlugin } from "./plan-tools.js";
import { contextPlugin } from "./context-tools.js";

registerPlugin(corePlugin);
registerPlugin(sessionPlugin);
registerPlugin(suggestionPlugin);
registerPlugin(planPlugin);
registerPlugin(contextPlugin);

const server = new McpServer({
    name: "fubbik",
    version: "0.0.1"
});

loadAllPlugins(server);

const transport = new StdioServerTransport();
await server.connect(transport);

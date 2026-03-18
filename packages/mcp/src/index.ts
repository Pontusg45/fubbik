#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

import { registerTools } from "./tools.js";
import { registerSessionTools } from "./session-tools.js";

const server = new McpServer({
    name: "fubbik",
    version: "0.0.1"
});

registerTools(server);
registerSessionTools(server);

const transport = new StdioServerTransport();
await server.connect(transport);

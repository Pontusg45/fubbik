import { execFile } from "node:child_process";
import { getPreferenceValues } from "@raycast/api";

const DEFAULT_URL = "http://127.0.0.1:3100/mcp";

/** Calls one of the desktop app's MCP tools and returns its JSON result. */
export async function callTool<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  const { mcpUrl } = getPreferenceValues<{ mcpUrl?: string }>();
  let response: Response;
  try {
    response = await fetch(mcpUrl || DEFAULT_URL, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }),
    });
  } catch {
    throw new Error("The fubbik desktop app is not running");
  }
  const body = (await response.json()) as {
    result?: { content?: { type: string; text: string }[]; isError?: boolean };
    error?: { message: string };
  };
  if (body.error) throw new Error(body.error.message);
  const text = body.result?.content?.find((c) => c.type === "text")?.text ?? "null";
  if (body.result?.isError) throw new Error(text);
  try {
    return JSON.parse(text) as T;
  } catch {
    return text as T;
  }
}

/** Brings the desktop app forward. */
export async function openApp() {
  await new Promise<void>((resolve, reject) =>
    execFile("/usr/bin/open", ["-b", "dev.fubbik.desktop"], (error) => (error ? reject(error) : resolve())),
  );
}

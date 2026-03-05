import { createStartHandler, defaultStreamHandler } from "@tanstack/react-start/server";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, extname } from "node:path";

const handler = createStartHandler(defaultStreamHandler);

const MIME_TYPES: Record<string, string> = {
  ".js": "application/javascript",
  ".css": "text/css",
  ".html": "text/html",
  ".json": "application/json",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".ttf": "font/ttf",
  ".txt": "text/plain",
};

function getClientDir(): string {
  // In production, dist/client is relative to dist/server/server.js
  const prodPath = join(import.meta.dirname, "..", "client");
  if (existsSync(prodPath)) return prodPath;
  return join(process.cwd(), "dist", "client");
}

function collectStaticFiles(dir: string, base = ""): Map<string, { path: string; mime: string }> {
  const files = new Map<string, { path: string; mime: string }>();
  if (!existsSync(dir)) return files;

  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const urlPath = base ? `${base}/${entry}` : `/${entry}`;
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      for (const [k, v] of collectStaticFiles(fullPath, urlPath)) {
        files.set(k, v);
      }
    } else {
      const ext = extname(entry);
      const mime = MIME_TYPES[ext] || "application/octet-stream";
      files.set(urlPath, { path: fullPath, mime });
    }
  }
  return files;
}

const clientDir = getClientDir();
const staticFiles = collectStaticFiles(clientDir);

export default {
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    // Serve static assets from dist/client/
    const staticFile = staticFiles.get(url.pathname);
    if (staticFile) {
      const content = readFileSync(staticFile.path);
      const headers: Record<string, string> = {
        "Content-Type": staticFile.mime,
      };
      // Cache hashed assets for 1 year
      if (url.pathname.includes("/assets/")) {
        headers["Cache-Control"] = "public, max-age=31536000, immutable";
      }
      return new Response(content, { headers });
    }

    // SSR handler for everything else
    return handler(request);
  },
};

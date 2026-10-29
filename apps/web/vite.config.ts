import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

const API_PROXY_TARGET = process.env.API_PROXY_TARGET ?? "http://127.0.0.1:3000";

const REPO_ROOT = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig(({ mode }) => {
    // Extra hostnames (e.g. a local HTTPS proxy domain) come from the root .env; localhost is always allowed.
    const allowedHosts = (loadEnv(mode, REPO_ROOT, "").FUBBIK_WEB_ALLOWED_HOSTS ?? "")
        .split(",")
        .map(host => host.trim())
        .filter(Boolean);
    return {
        plugins: [
            tsconfigPaths(),
            tailwindcss(),
            tanstackRouter({
                target: "react",
                quoteStyle: "double",
                semicolons: true,
                routeTreeFileHeader: ["/* eslint-disable */", "", "// @ts-nocheck", "", "// noinspection JSUnusedGlobalSymbols"]
            }),
            viteReact()
        ],
        server: {
            port: 3001,
            allowedHosts,
            hmr: {
                clientPort: 3001,
                host: "localhost"
            },
            watch: {
                ignored: ["**/routeTree.gen.ts"]
            },
            proxy: {
                "/api": {
                    target: API_PROXY_TARGET,
                    changeOrigin: true
                },
                "/docs": {
                    target: API_PROXY_TARGET,
                    changeOrigin: true
                }
            }
        }
    };
});

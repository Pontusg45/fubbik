import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

const API_PROXY_TARGET = process.env.API_PROXY_TARGET ?? "http://127.0.0.1:3000";

export default defineConfig({
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
        allowedHosts: ["app.fubbik.test"],
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
});

import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

const API_PROXY_TARGET = process.env.API_PROXY_TARGET ?? "http://127.0.0.1:3000";

export default defineConfig({
    plugins: [
        tsconfigPaths(),
        tailwindcss(),
        tanstackStart({
            server: { entry: "./entry-server" },
            router: {
                quoteStyle: "double",
                semicolons: true,
                routeTreeFileHeader: ["/* eslint-disable */", "", "// @ts-nocheck", "", "// noinspection JSUnusedGlobalSymbols"]
            }
        }),
        viteReact()
    ],
    define: {
        "import.meta.env.SSR_API_ORIGIN": JSON.stringify(API_PROXY_TARGET)
    },
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

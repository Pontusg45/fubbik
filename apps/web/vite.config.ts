import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

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
    server: {
        port: 3001,
        allowedHosts: ["app.fubbik.test"]
    }
});

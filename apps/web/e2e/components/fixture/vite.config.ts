import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
    root: new URL(".", import.meta.url).pathname,
    plugins: [tailwindcss(), react()],
    resolve: { alias: { "@": new URL("../../../src", import.meta.url).pathname } },
    server: { host: "127.0.0.1", port: 4178, strictPort: true }
});

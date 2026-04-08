#!/bin/sh

# Replace the build-time VITE_SERVER_URL placeholder with the runtime value
# This allows the same image to work in any environment
if [ -n "$RUNTIME_SERVER_URL" ]; then
    echo "Replacing server URL with: $RUNTIME_SERVER_URL"
    find /app/apps/web/dist -name '*.js' -exec sed -i "s|__FUBBIK_SERVER_URL__|${RUNTIME_SERVER_URL}|g" {} +
    find /app/apps/web/dist -name '*.js' -exec sed -i "s|http://localhost:3000|${RUNTIME_SERVER_URL}|g" {} +
fi

exec bun run dist/server/entry-server.js

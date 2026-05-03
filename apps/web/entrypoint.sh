#!/bin/sh

# Replace the build-time VITE_SERVER_URL placeholder with the runtime value
# This allows the same image to work in any environment
if [ -n "$RUNTIME_SERVER_URL" ]; then
    echo "Replacing server URL with: $RUNTIME_SERVER_URL"
    bun -e "
const fs = require('fs');
const path = require('path');

function walkDir(dir) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    const files = [];
    for (const entry of entries) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) files.push(...walkDir(full));
        else if (entry.name.endsWith('.js')) files.push(full);
    }
    return files;
}

const serverUrl = process.env.RUNTIME_SERVER_URL;
const files = walkDir('/app/dist');
let replaced = 0;
for (const f of files) {
    const content = fs.readFileSync(f, 'utf8');
    if (content.includes('__FUBBIK_SERVER_URL__') || content.includes('http://localhost:3000')) {
        fs.writeFileSync(f, content
            .replaceAll('__FUBBIK_SERVER_URL__', serverUrl)
            .replaceAll('http://localhost:3000', serverUrl));
        replaced++;
    }
}
console.log('  Replaced in ' + replaced + ' files');
"
fi

exec bun run dist/server/entry-server.js

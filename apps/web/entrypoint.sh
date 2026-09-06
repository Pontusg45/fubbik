#!/usr/bin/env sh

# Replace build-time API URL placeholders with the runtime value so one image
# works in any environment.
if [ -n "$RUNTIME_API_ORIGIN" ]; then
    echo "Replacing API origin with: $RUNTIME_API_ORIGIN"
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

const origin = process.env.RUNTIME_API_ORIGIN.replace(/\\/$/, '');
const legacy = process.env.RUNTIME_API_ORIGIN;
const files = walkDir('/app/dist');
let replaced = 0;
for (const f of files) {
    let content = fs.readFileSync(f, 'utf8');
    let next = content
        .replaceAll('http://fubbik-api.invalid', origin)
        .replaceAll('http://fubbik-legacy-api.invalid', legacy)
        .replaceAll('__FUBBIK_API_ORIGIN__', origin)
        .replaceAll('__FUBBIK_SERVER_URL__', legacy)
        .replaceAll('http://localhost:3000', legacy)
        .replaceAll('http://127.0.0.1:3000', legacy);
    if (next !== content) {
        fs.writeFileSync(f, next);
        replaced++;
    }
}
console.log('  Replaced in ' + replaced + ' files');
"
fi

exec bun run dist/server/entry-server.js

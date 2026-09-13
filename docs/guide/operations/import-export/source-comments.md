# Documentation from source comments

The Rust CLI can extract JavaScript JSDoc, TypeScript doc comments and Java
Javadoc into searchable reference chunks and a browsable document. Source
files are read locally; the API receives a versioned JSON manifest.

## Extract and preview

```bash
fubbik docs extract ./my-js-project --language javascript --project my-library --preview
fubbik docs extract ./my-ts-project --language typescript --project my-library --preview
fubbik docs extract ./my-java-project --language java --project my-library --preview
```

Preview prints the manifest and does not contact the API. Extraction requires
Node.js, plus JSDoc for JavaScript, TypeDoc for TypeScript, or a JDK (17+) for
Java. Install JSDoc/TypeDoc in the source project's development dependencies
or put their executables on PATH. Fubbik never downloads them automatically.
TypeDoc uses the project's `typedoc.json` and `tsconfig.json`; configure its
entry points to cover the intended API. Java uses Javadoc's normal public and
protected visibility and requires dependencies to be available to Javadoc
(for example through `CLASSPATH`). Extractor failures stop before any import.

JavaScript and Java directory scans skip symlinks, `.git`, `node_modules`,
`target`, `build`, `dist`, and `.next`. The installed tools define supported
syntax and comment tags. Java block tags and inline Javadoc notation are
preserved as text; inherited comment expansion and conversion of every
Javadoc HTML/inline tag to Markdown are not implemented.

## Import and refresh

```bash
fubbik docs extract ./my-java-project --language java --project my-library --space my-space

# A CI job or custom extractor can also produce the manifest.
fubbik docs extract source-manifest.json --preview
fubbik docs extract source-manifest.json --space my-space
```

The same space, project name and language identify one generated document.
Use a distinct project name for each extraction scope; narrowing a complete
scan under the same name treats omitted symbols as removed. Symbol keys are
qualified Java declarations (including overload parameters), or relative file
paths plus qualified JavaScript/TypeScript names. Line changes preserve chunk
IDs; renames and JS/TS file moves are treated as removal plus addition.

Imports are atomic. Repeated identical imports reuse chunks. Changed comments
create version history. Missing symbols from a complete scan are archived;
reappearing symbols reuse their previous chunks. A manifest with `complete:
false` imports its symbols without archiving missing ones. Human changes to a
generated chunk's title/content produce a conflict and are preserved, including
when the source symbol disappears. Resolve the conflict by restoring generated
text or moving human annotations into chunk comments/separate linked chunks.
Tags and other human metadata are preserved.

With `--json`, the result reports the document ID, created/updated/unchanged/missing
counts, conflicting symbol keys, and diagnostics. Conflicts produce a nonzero
CLI exit status after reporting any successfully imported symbols. Source references connect
each chunk to its relative file path and symbol anchor. Browse the resulting
document through the existing Documents page, or use `fubbik docs render ID`.
Refresh with `docs extract`; ordinary Markdown `docs sync` is rejected for
generated source documents.

## Manifest version 1

```json
{
  "version": 1,
  "project": "my-library",
  "language": "java",
  "extractor": "my-doclet-v1",
  "complete": true,
  "diagnostics": [],
  "symbols": [{
    "key": "example.Lookup#find(int)",
    "title": "Lookup.find(int)",
    "signature": "String find(int id)",
    "documentation": "Finds a stored value.\n\n@param id Lookup identifier",
    "path": "src/example/Lookup.java",
    "line": 12,
    "references": []
  }]
}
```

Versions, duplicate keys, non-relative paths, and oversized fields are
validated before writes. A request supports at most 2,000 symbols and remains
subject to the HTTP request body limit. Use separate project scopes for larger
APIs. Keep the same extractor version/configuration in CI for stable output.

## Development checks

Run the real extractor fixtures with `node --test
crates/fubbik-cli/tests/source-docs.test.mjs` after installing the three tools.
Database behavior is covered by `cargo test -p fubbik-api --test source_docs`
with the Rust test database running.

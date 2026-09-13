// Runs installed documentation tools and normalizes their output. No downloads.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";

const [rootArg, language, project, temporary] = process.argv.slice(2);
const root = realpathSync(rootArg);
function run(executable, args) {
    const result = spawnSync(executable, args, { cwd: root, encoding: "utf8", timeout: 120000, maxBuffer: 32 * 1024 * 1024 });
    if (result.error || result.status !== 0) throw new Error(`${executable} failed: ${result.error?.message ?? result.stderr}`);
    if (result.stderr.trim()) process.stderr.write(result.stderr);
    return result.stdout;
}
function binary(name) {
    let directory = root;
    while (true) {
        const candidate = join(directory, "node_modules", ".bin", name);
        if (existsSync(candidate)) return candidate;
        const parent = dirname(directory);
        if (parent === directory) return name;
        directory = parent;
    }
}
function sourcePath(path) {
    const value = relative(root, resolve(root, path)).replaceAll("\\", "/");
    if (value.startsWith("../") || !value) throw new Error(`Source is outside extraction root: ${path}`);
    return value;
}
function sourceFiles(extensions) {
    const files = [];
    function walk(directory) {
        for (const entry of readdirSync(directory, { withFileTypes: true })) {
            if (entry.isSymbolicLink() || [".git", "node_modules", "target", "build", "dist", ".next"].includes(entry.name)) continue;
            const path = join(directory, entry.name);
            if (entry.isDirectory()) walk(path);
            else if (extensions.some(extension => entry.name.endsWith(extension))) files.push(path);
        }
    }
    walk(root);
    if (!files.length) throw new Error("No matching source files found");
    return files.sort();
}
const text = value => (typeof value === "string" ? value : Array.isArray(value) ? value.map(p => p.text ?? "").join("") : "");
const section = (label, value) => (value ? `\n\n### ${label}\n\n${value}` : "");
const symbols = [];
let extractor;
if (language === "javascript") {
    const tool = binary("jsdoc");
    extractor = `jsdoc ${run(tool, ["--version"]).trim()}`;
    const doclets = JSON.parse(run(tool, ["-X", ...sourceFiles([".js", ".mjs", ".cjs"])]));
    for (const d of doclets) {
        if (d.undocumented || d.ignore || d.access === "private" || !d.meta?.filename || !d.longname || d.kind === "package") continue;
        const path = sourcePath(join(d.meta.path ?? root, d.meta.filename));
        const params = d.params ?? [];
        const type = p => p.type?.names?.join(" | ") ?? "";
        const returns = (d.returns ?? []).map(p => `${type(p)} ${p.description ?? ""}`).join("\n");
        const documentation =
            (d.description ?? d.classdesc ?? "") +
            section(
                "Parameters",
                params.map(p => `- **${p.name}**${p.optional ? " (optional)" : ""}: ${type(p)} ${p.description ?? ""}`).join("\n")
            ) +
            section("Returns", returns) +
            section("Throws", (d.exceptions ?? []).map(p => `${type(p)} ${p.description ?? ""}`).join("\n")) +
            section("Examples", (d.examples ?? []).join("\n\n")) +
            section("Deprecated", d.deprecated === true ? "Deprecated." : d.deprecated) +
            section("See also", (d.see ?? []).join("\n"));
        symbols.push({
            key: `${path}::${d.longname}`,
            title: d.longname,
            signature: `${d.longname}${["function", "class"].includes(d.kind) ? `(${params.map(p => `${p.name}${p.optional ? "?" : ""}${type(p) ? `: ${type(p)}` : ""}`).join(", ")})` : ""}${d.returns?.length ? `: ${d.returns.map(type).join(" | ")}` : ""}`,
            documentation,
            path,
            line: d.meta.lineno ?? 1,
            references: d.see ?? []
        });
    }
} else if (language === "typescript") {
    const tool = binary("typedoc");
    extractor = run(tool, ["--version"]).trim();
    const output = join(temporary, "typedoc.json");
    // Project configuration controls entry points, visibility and resolution.
    run(tool, ["--json", output]);
    const tree = JSON.parse(readFileSync(output, "utf8"));
    function comment(c) {
        if (!c) return "";
        return (
            text(c.summary) +
            (c.blockTags ?? []).map(t => section(t.tag.replace(/^@/, ""), text(t.content))).join("") +
            (c.modifierTags?.includes("@deprecated") ? section("Deprecated", "Deprecated.") : "")
        );
    }
    function type(t) {
        if (!t) return "unknown";
        if (t.type === "namedTupleMember") return `${t.name}${t.isOptional ? "?" : ""}: ${type(t.element)}`;
        if (t.name) return t.name + (t.typeArguments?.length ? `<${t.typeArguments.map(type).join(", ")}>` : "");
        if (t.type === "array") return `${type(t.elementType)}[]`;
        if (t.type === "tuple") return `[${(t.elements ?? []).map(type).join(", ")}]`;
        if (t.type === "typeOperator") return `${t.operator} ${type(t.target)}`;
        if (t.type === "optional") return `${type(t.elementType)}?`;
        if (t.type === "rest") return `...${type(t.elementType)}`;
        if (t.types) return t.types.map(type).join(t.type === "intersection" ? " & " : " | ");
        if (t.type === "literal") return JSON.stringify(t.value);
        return t.type;
    }
    function visit(node, parents = []) {
        const names = [...parents, node.name];
        const qualified = names.join(".");
        const declarations = node.signatures?.length ? node.signatures : [node];
        for (const d of declarations) {
            const c = d.comment ?? node.comment;
            const source = d.sources?.[0] ?? node.sources?.[0];
            if (!c || !source || node.flags?.isPrivate || d.flags?.isPrivate) continue;
            const path = sourcePath(source.fullFileName ?? source.fileName);
            const params = d.parameters ?? [];
            const generics = d.typeParameters?.length
                ? `<${d.typeParameters.map(p => `${p.name}${p.type ? ` extends ${type(p.type)}` : ""}`).join(", ")}>`
                : "";
            const signature = `${qualified}${generics}${node.signatures ? `(${params.map(p => `${p.name}${p.flags?.isOptional ? "?" : ""}: ${type(p.type)}`).join(", ")})` : ""}: ${type(d.type)}`;
            symbols.push({
                key: `${path}::${qualified}${node.signatures ? `(${params.map(p => type(p.type)).join(",")})` : ""}`,
                title: qualified,
                signature,
                documentation: comment(c) + section("Parameters", params.map(p => `- **${p.name}**: ${comment(p.comment)}`).join("\n")),
                path,
                line: source.line,
                references: []
            });
        }
        for (const child of node.children ?? []) visit(child, names);
    }
    for (const child of tree.children ?? []) visit(child);
} else if (language === "java") {
    const files = sourceFiles([".java"]);
    run("javac", ["-d", temporary, join(temporary, "FubbikDoclet.java")]);
    const output = join(temporary, "java.json");
    run("javadoc", [
        "-quiet",
        "-doclet",
        "FubbikDoclet",
        "-docletpath",
        temporary,
        "-fubbik-output",
        output,
        "-fubbik-root",
        root,
        ...files.sort()
    ]);
    symbols.push(...JSON.parse(readFileSync(output, "utf8")));
    extractor = run("java", ["--version"]).split("\n")[0];
} else throw new Error(`Unsupported language ${language}`);

symbols.sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
process.stdout.write(JSON.stringify({ version: 1, project, language, extractor, complete: true, diagnostics: [], symbols }, null, 2));

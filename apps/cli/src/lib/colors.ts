import pc from "picocolors";

export const formatSuccess = (msg: string) => `${pc.green("\u2713")} ${msg}`;
export const formatError = (msg: string) => `${pc.red("\u2717")} ${msg}`;
export const formatDim = (msg: string) => pc.dim(msg);
export const formatBold = (msg: string) => pc.bold(msg);
export const formatType = (type: string) => pc.cyan(`(${type})`);
export const formatId = (id: string) => pc.dim(id);
export const formatTag = (tag: string) => pc.yellow(tag);
export const formatTitle = (title: string) => pc.bold(title);
export const formatRelation = (rel: string) => pc.magenta(rel);
export const formatScore = (score: string) => pc.green(score);

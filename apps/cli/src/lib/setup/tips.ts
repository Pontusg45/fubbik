import type { Tip } from "./types";

export function mergeTips(tips: Tip[]): Tip[] {
	const seen = new Map<string, Tip>();
	for (const tip of tips) {
		if (!seen.has(tip.title)) {
			seen.set(tip.title, tip);
		}
	}
	return [...seen.values()];
}

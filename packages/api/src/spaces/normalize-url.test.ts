import { describe, expect, it } from "vitest";

import { normalizeGitUrl } from "./normalize-url";

describe("normalizeGitUrl", () => {
    it("strips .git suffix", () => {
        expect(normalizeGitUrl("https://github.com/user/repo.git")).toBe("github.com/user/repo");
    });
    it("strips trailing slashes", () => {
        expect(normalizeGitUrl("https://github.com/user/repo/")).toBe("github.com/user/repo");
    });
    it("normalizes SSH to path form", () => {
        expect(normalizeGitUrl("git@github.com:user/repo.git")).toBe("github.com/user/repo");
    });
    it("normalizes HTTPS", () => {
        expect(normalizeGitUrl("https://github.com/user/repo")).toBe("github.com/user/repo");
    });
    it("normalizes HTTP", () => {
        expect(normalizeGitUrl("http://github.com/user/repo")).toBe("github.com/user/repo");
    });
    it("handles ssh:// protocol", () => {
        expect(normalizeGitUrl("ssh://git@github.com/user/repo.git")).toBe("github.com/user/repo");
    });
    it("SSH and HTTPS for same repo produce same result", () => {
        const ssh = normalizeGitUrl("git@github.com:user/repo.git");
        const https = normalizeGitUrl("https://github.com/user/repo.git");
        expect(ssh).toBe(https);
    });
});

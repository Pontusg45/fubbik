import { describe, expect, it } from "vitest";

import { normalizeGitUrl } from "./normalize-url";

describe("normalizeGitUrl", () => {
    it("strips .git suffix", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("https://github.com/user/repo.git")).toBe("github.com/user/repo");
    });
    it("strips trailing slashes", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("https://github.com/user/repo/")).toBe("github.com/user/repo");
    });
    it("normalizes SSH to path form", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("git@github.com:user/repo.git")).toBe("github.com/user/repo");
    });
    it("normalizes HTTPS", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("https://github.com/user/repo")).toBe("github.com/user/repo");
    });
    it("normalizes HTTP", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("http://github.com/user/repo")).toBe("github.com/user/repo");
    });
    it("handles ssh:// protocol", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(normalizeGitUrl("ssh://git@github.com/user/repo.git")).toBe("github.com/user/repo");
    });
    it("SSH and HTTPS for same repo produce same result", () => {
        // Given
        const ssh = normalizeGitUrl("git@github.com:user/repo.git");
        // When
        const https = normalizeGitUrl("https://github.com/user/repo.git");
        // Then
        expect(ssh).toBe(https);
    });
});

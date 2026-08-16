import { describe, expect, it } from "vitest";
import { crossSubDomainCookies } from "../src/cross-subdomain-cookie";

describe("crossSubDomainCookies", () => {
    it("is a no-op when AUTH_COOKIE_DOMAIN is unset, regardless of host", () => {
        expect(crossSubDomainCookies("https://api.fubbik.test", undefined)).toBeUndefined();
    });

    it("yields no Domain attribute for localhost even with an override set", () => {
        expect(crossSubDomainCookies("http://localhost:3000", ".fubbik.test")).toBeUndefined();
    });

    it("yields no Domain attribute for a bare IPv4 literal", () => {
        expect(crossSubDomainCookies("http://127.0.0.1:3000", ".fubbik.test")).toBeUndefined();
    });

    it("yields no Domain attribute for an IPv6 literal", () => {
        expect(crossSubDomainCookies("http://[::1]:3000", ".fubbik.test")).toBeUndefined();
    });

    it("strips the port and honors the override for a URL with a port", () => {
        expect(crossSubDomainCookies("https://api.fubbik.test:8443", ".fubbik.test")).toEqual({
            enabled: true,
            domain: ".fubbik.test"
        });
    });

    it("honors the override for a three-label host", () => {
        expect(crossSubDomainCookies("https://api.fubbik.test", ".fubbik.test")).toEqual({
            enabled: true,
            domain: ".fubbik.test"
        });
    });

    it("honors the override for a two-label apex host (the case slice(-2) still broke)", () => {
        expect(crossSubDomainCookies("https://fubbik.test", ".fubbik.test")).toEqual({
            enabled: true,
            domain: ".fubbik.test"
        });
    });

    it("does not guess a public suffix like .co.uk as the registrable domain", () => {
        expect(crossSubDomainCookies("https://app.example.co.uk", ".example.co.uk")).toEqual({
            enabled: true,
            domain: ".example.co.uk"
        });
    });

    it("honors the override for a deeply-nested subdomain rather than deriving the last two labels", () => {
        expect(crossSubDomainCookies("https://a.b.example.com", ".example.com")).toEqual({
            enabled: true,
            domain: ".example.com"
        });
    });

    it("is unaffected by a trailing dot in the hostname", () => {
        expect(crossSubDomainCookies("https://fubbik.test.", ".fubbik.test")).toEqual({
            enabled: true,
            domain: ".fubbik.test"
        });
    });

    it("yields no Domain attribute for a malformed URL", () => {
        expect(crossSubDomainCookies("not a url", ".fubbik.test")).toBeUndefined();
    });
});

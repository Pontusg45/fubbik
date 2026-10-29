/**
 * The session cookie has to reach two origins: Node (which issues it) and the
 * Rust API (which only verifies it). Under the Caddy setup those are sibling
 * subdomains — `api.fubbik.test` and `rs.fubbik.test` — and a cookie with no
 * `Domain` attribute is host-only per RFC 6265, so it would never be sent to
 * the second one.
 *
 * The registrable domain (`fubbik.test` vs. `co.uk` vs. `example.com`) is
 * NOT guessable from label counts — `hostname.split(".").slice(-2)` yields a
 * public suffix like `.co.uk` for `app.example.co.uk`, which browsers reject
 * outright, and silently widens the cookie's scope to every `*.example.com`
 * host on an ordinary three-label production domain. Multi-part public
 * suffixes (`.co.uk`, `.com.au`, `.github.io`, `.vercel.app`, ...) make this
 * undecidable without a public-suffix list, and this repo doesn't depend on
 * one.
 *
 * So this is opt-in, not inferred: it does nothing unless `authCookieDomain`
 * (normally `process.env.AUTH_COOKIE_DOMAIN`) is explicitly set — e.g.
 * `.fubbik.test` for the Caddy setup. That is the only place that needs
 * cross-subdomain cookies at all.
 *
 * Even with an explicit override, some hosts must never get a `Domain`
 * attribute:
 *
 *   - `localhost` — no parent, and none needed. Cookies are NOT isolated by
 *     port, so Node on :3000 and Rust on :3100 already share them. Setting
 *     `Domain=localhost` here would be worse than useless: browsers reject
 *     it.
 *   - a bare IP (`127.0.0.1`, `[::1]`) — has no registrable parent; sending
 *     a `Domain` attribute for one is invalid and would silently break auth.
 *
 * so those are guarded regardless of the override, as defense-in-depth
 * against a misconfigured env var reaching a dev/loopback deployment.
 */
export function crossSubDomainCookies(
    betterAuthUrl: string,
    authCookieDomain: string | undefined = process.env.AUTH_COOKIE_DOMAIN
): { enabled: true; domain: string } | undefined {
    if (!authCookieDomain) {
        return undefined;
    }

    let hostname: string;
    try {
        hostname = new URL(betterAuthUrl).hostname;
    } catch {
        return undefined;
    }

    const labels = hostname.split(".");
    const isIpv4Literal = labels.length === 4 && labels.every(l => /^\d+$/.test(l));
    const isIpv6Literal = hostname.includes(":");
    if (isIpv4Literal || isIpv6Literal || hostname === "localhost") {
        return undefined;
    }

    return { enabled: true as const, domain: authCookieDomain };
}

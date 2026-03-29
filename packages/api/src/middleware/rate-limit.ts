const windows = new Map<string, { count: number; resetAt: number }>();

export function checkRateLimit(
    key: string,
    maxRequests: number,
    windowMs: number
): { allowed: boolean; remaining: number; resetAt: number } {
    const now = Date.now();
    const entry = windows.get(key);

    if (!entry || now > entry.resetAt) {
        windows.set(key, { count: 1, resetAt: now + windowMs });
        return { allowed: true, remaining: maxRequests - 1, resetAt: now + windowMs };
    }

    entry.count++;
    const allowed = entry.count <= maxRequests;
    return { allowed, remaining: Math.max(0, maxRequests - entry.count), resetAt: entry.resetAt };
}

// Cleanup old entries every 5 minutes
setInterval(() => {
    const now = Date.now();
    for (const [key, entry] of windows) {
        if (now > entry.resetAt) windows.delete(key);
    }
}, 5 * 60 * 1000);

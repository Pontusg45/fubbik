import { useEffect, useRef } from "react";

export function useIntersectionObserver(callback: () => void, enabled: boolean) {
    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (!enabled || !ref.current) return;
        const observer = new IntersectionObserver(
            ([entry]) => {
                if (entry?.isIntersecting) callback();
            },
            { rootMargin: "200px" }
        );
        observer.observe(ref.current);
        return () => observer.disconnect();
    }, [callback, enabled]);
    return ref;
}

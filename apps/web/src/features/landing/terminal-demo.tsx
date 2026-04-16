import { useCallback, useEffect, useRef, useState } from "react";

interface Line {
  text: string;
  type: "command" | "output";
}

const DEMO_LINES: Line[] = [
  {
    text: '$ fubbik quick "Always use Effect for typed errors"',
    type: "command",
  },
  { text: "✓ Created a8f3 — note", type: "output" },
  { text: "", type: "output" }, // blank separator
  { text: '$ fubbik search "error handling"', type: "command" },
  { text: "3 chunks found across 2 codebases", type: "output" },
  { text: "", type: "output" }, // blank separator
  { text: "$ fubbik context --for src/api/auth.ts", type: "command" },
  { text: "Found 5 relevant chunks (2,400 tokens)", type: "output" },
];

const CHAR_DELAY = 25;
const OUTPUT_DELAY = 750;
const BLANK_LINE_DELAY = 400;
const LOOP_PAUSE = 3000;

export function TerminalDemo() {
  const [visibleLines, setVisibleLines] = useState<
    { text: string; type: "command" | "output" }[]
  >([]);
  const [typingIndex, setTypingIndex] = useState(0);
  const [isAnimating, setIsAnimating] = useState(false);
  const [cursorVisible, setCursorVisible] = useState(true);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const animationRef = useRef<{ cancel: boolean }>({ cancel: false });

  // Detect reduced motion preference
  useEffect(() => {
    const mql = window.matchMedia("(prefers-reduced-motion: reduce)");
    setPrefersReducedMotion(mql.matches);
    const handler = (e: MediaQueryListEvent) => setPrefersReducedMotion(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  // Blinking cursor
  useEffect(() => {
    if (prefersReducedMotion) return;
    const interval = setInterval(() => setCursorVisible((v) => !v), 530);
    return () => clearInterval(interval);
  }, [prefersReducedMotion]);

  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  const runAnimation = useCallback(async () => {
    const token = { cancel: false };
    animationRef.current = token;

    while (!token.cancel) {
      setVisibleLines([]);
      setTypingIndex(-1);

      for (let i = 0; i < DEMO_LINES.length; i++) {
        if (token.cancel) return;
        const line = DEMO_LINES[i];

        if (line!.type === "command") {
          // Typewriter effect for commands
          setTypingIndex(i);
          for (let c = 0; c <= line!.text.length; c++) {
            if (token.cancel) return;
            setVisibleLines((prev) => {
              const next = prev.slice(0, i);
              next.push({ text: line!.text.slice(0, c), type: "command" });
              return next;
            });
            await sleep(CHAR_DELAY);
          }
          setTypingIndex(-1);
          // Delay before output appears
          await sleep(OUTPUT_DELAY);
        } else if (line!.text === "") {
          // Blank separator line
          setVisibleLines((prev) => [...prev, { text: "", type: "output" }]);
          await sleep(BLANK_LINE_DELAY);
        } else {
          // Output lines appear instantly
          setVisibleLines((prev) => [...prev, { text: line!.text, type: "output" }]);
        }
      }

      // Pause before looping
      await sleep(LOOP_PAUSE);
    }
  }, []);

  // IntersectionObserver to auto-play
  useEffect(() => {
    if (prefersReducedMotion) {
      // Show all lines immediately
      setVisibleLines(DEMO_LINES.map((l) => ({ text: l.text, type: l.type })));
      return;
    }

    const el = containerRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry!.isIntersecting && !isAnimating) {
          setIsAnimating(true);
          runAnimation();
        } else if (!entry!.isIntersecting && isAnimating) {
          animationRef.current.cancel = true;
          setIsAnimating(false);
        }
      },
      { threshold: 0.3 },
    );

    observer.observe(el);
    return () => {
      observer.disconnect();
      animationRef.current.cancel = true;
    };
  }, [prefersReducedMotion, isAnimating, runAnimation]);

  return (
    <div ref={containerRef} className="mx-auto max-w-lg">
      <div className="overflow-hidden rounded-lg border border-white/10 bg-[#0d1117]">
        {/* Title bar */}
        <div className="flex items-center gap-2 border-b border-white/10 px-4 py-2.5">
          <div className="flex gap-1.5">
            <div className="h-3 w-3 rounded-full bg-white/10" />
            <div className="h-3 w-3 rounded-full bg-white/10" />
            <div className="h-3 w-3 rounded-full bg-white/10" />
          </div>
          <span className="ml-2 text-xs text-white/40">terminal</span>
        </div>

        {/* Terminal content */}
        <div className="p-4 font-mono text-[13px] leading-relaxed">
          {visibleLines.map((line, i) => {
            if (line.text === "" && line.type === "output") {
              return <div key={i} className="h-4" />;
            }

            const isCurrentlyTyping = typingIndex === i;

            return (
              <div key={i} className="min-h-[1.625rem]">
                <span
                  className={
                    line.type === "command"
                      ? "text-foreground"
                      : "text-emerald-400"
                  }
                >
                  {line.text}
                </span>
                {isCurrentlyTyping && !prefersReducedMotion && (
                  <span
                    className={`text-foreground ${cursorVisible ? "opacity-100" : "opacity-0"}`}
                  >
                    ▊
                  </span>
                )}
              </div>
            );
          })}
          {/* Show cursor on empty terminal waiting to start */}
          {visibleLines.length === 0 && !prefersReducedMotion && (
            <div className="min-h-[1.625rem]">
              <span
                className={`text-foreground ${cursorVisible ? "opacity-100" : "opacity-0"}`}
              >
                ▊
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

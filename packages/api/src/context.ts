import type { auth } from "@fubbik/auth";

export type Session = Awaited<ReturnType<(typeof auth)["api"]["getSession"]>>;

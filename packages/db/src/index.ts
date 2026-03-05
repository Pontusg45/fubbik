import { env } from "@fubbik/env/server";
import { drizzle } from "drizzle-orm/node-postgres";

import * as schema from "./schema/index.js";

export const db = drizzle(env.DATABASE_URL, { schema });

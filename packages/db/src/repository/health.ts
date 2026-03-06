import { sql } from "drizzle-orm";
import { db } from "../index";

export async function checkDbConnectivity(): Promise<boolean> {
  await db.execute(sql`SELECT 1`);
  return true;
}

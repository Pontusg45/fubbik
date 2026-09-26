import { authClient } from "@/lib/auth-client";

/** Resolve the same session shape through the Rust API in the browser. */
export async function getUser() {
    const { data, error } = await authClient.getSession();
    if (error) throw error;
    return data;
}

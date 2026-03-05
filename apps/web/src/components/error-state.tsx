import { type ErrorComponentProps, Link, useRouter } from "@tanstack/react-router";
import { AlertTriangle, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";

export default function ErrorState({ error, reset }: ErrorComponentProps) {
  const router = useRouter();

  const message = error instanceof Error ? error.message : "An unexpected error occurred";

  const isNetworkError =
    message.includes("fetch") || message.includes("network") || message.includes("Failed to fetch");

  return (
    <div className="flex min-h-svh flex-col items-center justify-center gap-4 px-4 py-16">
      <AlertTriangle className="text-destructive size-10" />
      <div className="text-center">
        <h1 className="text-2xl font-bold tracking-tight">
          {isNetworkError ? "Connection failed" : "Something went wrong"}
        </h1>
        <p className="text-muted-foreground mt-1 max-w-md text-sm">
          {isNetworkError
            ? "Could not reach the server. Check that the API is running and try again."
            : message}
        </p>
      </div>
      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={() => {
            reset?.();
            router.invalidate();
          }}
        >
          <RefreshCw className="size-4" />
          Try again
        </Button>
        <Button variant="ghost" render={<Link to="/" />}>
          Back to home
        </Button>
      </div>
    </div>
  );
}

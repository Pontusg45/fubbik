import { Link } from "@tanstack/react-router";
import { FileQuestion } from "lucide-react";

import { Button } from "@/components/ui/button";

export default function NotFound() {
    return (
        <div className="flex min-h-svh flex-col items-center justify-center gap-4 px-4 py-16">
            <FileQuestion className="text-muted-foreground size-10" />
            <div className="text-center">
                <h1 className="text-2xl font-bold tracking-tight">Page not found</h1>
                <p className="text-muted-foreground mt-1 text-sm">The page you're looking for doesn't exist or has been moved.</p>
            </div>
            <Button variant="outline" render={<Link to="/" />}>
                Back to home
            </Button>
        </div>
    );
}

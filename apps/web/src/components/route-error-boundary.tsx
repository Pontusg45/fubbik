import { AlertTriangle } from "lucide-react";
import { Component, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { PageContainer } from "@/components/ui/page";

interface Props {
    children: ReactNode;
    fallbackTitle?: string;
}

interface State {
    hasError: boolean;
    error: Error | null;
}

export class RouteErrorBoundary extends Component<Props, State> {
    constructor(props: Props) {
        super(props);
        this.state = { hasError: false, error: null };
    }

    static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error };
    }

    render() {
        if (this.state.hasError) {
            return (
                <PageContainer>
                    <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4">
                        <AlertTriangle className="text-destructive h-10 w-10" />
                        <h2 className="text-lg font-semibold">
                            {this.props.fallbackTitle ?? "Something went wrong"}
                        </h2>
                        <p className="text-muted-foreground max-w-md text-center text-sm">
                            {this.state.error?.message ?? "An unexpected error occurred."}
                        </p>
                        <Button onClick={() => this.setState({ hasError: false, error: null })}>
                            Try again
                        </Button>
                    </div>
                </PageContainer>
            );
        }
        return this.props.children;
    }
}

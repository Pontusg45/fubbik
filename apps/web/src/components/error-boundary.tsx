import { Component, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
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
        this.props.fallback ?? (
          <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4">
            <h2 className="text-lg font-semibold">Something went wrong</h2>
            <p className="text-muted-foreground text-sm">
              {this.state.error?.message ?? "An unexpected error occurred."}
            </p>
            <Button onClick={() => this.setState({ hasError: false, error: null })}>
              Try again
            </Button>
          </div>
        )
      );
    }
    return this.props.children;
  }
}

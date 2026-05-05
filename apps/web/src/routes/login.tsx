import { createFileRoute, Navigate } from "@tanstack/react-router";
import { useState } from "react";

import SignInForm from "@/features/auth/sign-in-form";
import SignUpForm from "@/features/auth/sign-up-form";
import { isImplicitDevUxEnabled } from "@/lib/implicit-dev-ux";

export const Route = createFileRoute("/login")({
    component: RouteComponent
});

function RouteComponent() {
    const [showSignIn, setShowSignIn] = useState(false);

    if (isImplicitDevUxEnabled()) {
        return <Navigate to="/dashboard" />;
    }

    return showSignIn ? (
        <SignInForm onSwitchToSignUp={() => setShowSignIn(false)} />
    ) : (
        <SignUpForm onSwitchToSignIn={() => setShowSignIn(true)} />
    );
}

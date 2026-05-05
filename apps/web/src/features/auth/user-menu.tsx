import { Link, useNavigate } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import { authClient } from "@/lib/auth-client";
import { IMPLICIT_DEV_USER_MENU, isImplicitDevUxEnabled } from "@/lib/implicit-dev-ux";

export default function UserMenu() {
    const navigate = useNavigate();
    const implicitUx = isImplicitDevUxEnabled();
    const { data: session, isPending } = authClient.useSession();

    const effectiveDisplayUser = session?.user ?? (implicitUx ? IMPLICIT_DEV_USER_MENU : undefined);

    if (!implicitUx && isPending) {
        return <Skeleton className="h-9 w-24" />;
    }

    if (!effectiveDisplayUser) {
        return (
            <Link to="/login">
                <Button variant="outline">Sign In</Button>
            </Link>
        );
    }

    const hasBetterAuthSession = Boolean(session);

    return (
        <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" />}>{effectiveDisplayUser.name}</DropdownMenuTrigger>
            <DropdownMenuContent className="bg-card">
                <DropdownMenuGroup>
                    <DropdownMenuLabel>My Account</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {implicitUx && !hasBetterAuthSession && (
                        <DropdownMenuItem className="text-muted-foreground focus:bg-muted cursor-default text-xs" disabled>
                            Signed in implicitly (local / self-host); no credentials required.
                        </DropdownMenuItem>
                    )}
                    <DropdownMenuItem>{effectiveDisplayUser.email}</DropdownMenuItem>
                    {hasBetterAuthSession ? (
                        <DropdownMenuItem
                            variant="destructive"
                            onClick={() => {
                                authClient.signOut({
                                    fetchOptions: {
                                        onSuccess: () => {
                                            navigate({
                                                to: "/"
                                            });
                                        }
                                    }
                                });
                            }}
                        >
                            Sign Out
                        </DropdownMenuItem>
                    ) : null}
                </DropdownMenuGroup>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}

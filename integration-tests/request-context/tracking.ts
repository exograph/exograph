import "./claytip.d.ts"

interface AuthContext {
    secretHeader: string,
}

export function shouldTrack(context: AuthContext): boolean {
    // don't track any users from localhost
    if (context.secretHeader == "pancake") {
        return false;
    }

    return true;
}
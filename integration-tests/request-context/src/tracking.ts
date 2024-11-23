interface AuthContext {
    secretHeader: string,
}

export function shouldTrack(context: AuthContext): boolean {
    console.log("auth context", JSON.stringify(context, null, 2));
    // don't track any users from localhost
    if (context.secretHeader == "pancake") {
        return false;
    }

    return true;
}
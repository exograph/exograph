import type { Exograph } from 'https://deno.land/x/exograph@v0.0.5/index.ts';

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
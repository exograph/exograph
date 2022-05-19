import "./claytip.d.ts"

interface AuthContext {
    role: string,
    connectingIp: string,
    devMode: string
}

export function shouldTrack(context: AuthContext): boolean {
    console.log(context)

    // don't track any users from localhost
    if (context.connectingIp == "127.0.0.1") {
        return false;
    }

    return true;
}
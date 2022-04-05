import "./claytip.d.ts";

interface AuthContext {
    role: string,
    connectingIp: string,
    devMode: string
}

export function getRole(context: AuthContext): string {
    return context.role
}

export function getConnectingIp(context: AuthContext): string {
    return context.connectingIp
}

export function getDevModeEnabled(context: AuthContext): boolean {
    return context.devMode == "1"
}

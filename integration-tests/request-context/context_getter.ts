import "./claytip.d.ts";

interface AuthContext {
    role: string,
    connectingIp: string,
    devMode: string
}

interface TrackingContext {
    uid: string,
    shouldTrack: boolean
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

export function isTrackingEnabled(context: TrackingContext): boolean {
    return context.shouldTrack
}

// two separate injected contexts

export function getRoleAndUid(auth: AuthContext, tracking: TrackingContext): string {
    return auth.role + "," + tracking.uid
}


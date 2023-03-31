import "./exograph.d.ts";

interface AuthContext {
    role: string,
    secretHeader: string,
    devMode: string,
    clientIp: string
}

interface TrackingContext {
    uid: string,
    shouldTrack: boolean
}

export function getRole(context: AuthContext): string {
    return context.role
}

export function getSecretHeader(context: AuthContext): string {
    return context.secretHeader
}

export function getDevModeEnabled(context: AuthContext): boolean {
    return context.devMode == "1"
}

export function getIp(context: AuthContext): string {
    return context.clientIp
}

export function isTrackingEnabled(context: TrackingContext): boolean {
    return context.shouldTrack
}

// two separate injected contexts

export function getRoleAndUid(auth: AuthContext, tracking: TrackingContext): string {
    return auth.role + "," + tracking.uid
}


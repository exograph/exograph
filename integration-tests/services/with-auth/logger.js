export function getRole(auth_context) {
    return auth_context["role"]
}

export function logNormal(entry) {
    console.log("[" + entry.level + "]: " + entry.message)
    return true
}

export function logPrivileged(entry) {
    console.log("!!! [" + entry.level + "]: " + entry.message)
    return true
}
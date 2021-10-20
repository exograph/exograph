export function add(x, y) {
    return x + y
}

export function divide(x, y) {
    let quotient = Math.floor(x / y);
    let remainder = x % y;

    return {
        "quotient": quotient,
        "remainder": remainder
    }
}

export function sideEffectQuery() {
    return 42   
}

export function sideEffectMutation() {
    return 3.14
}

export function log(env, message) {
    console.log(message)
    return true
}
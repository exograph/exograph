export function divide(x, y) {
    let quotient = Math.floor(x / y);
    let remainder = x % y;

    if (y == 0) {
        throw new ExographError("Division by zero is not allowed")
    }

    return {
        "quotient": quotient,
        "remainder": remainder
    }
}

export async function asyncDivide(x, y) {
    return divide(x, y);
}

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

export function shimQuery(claytip) {
    // c 

    let result = claytip.executeQuery(
        `query {
            foos(
                where: { baz: {eq: 4} }
            ) {
                id
            }
        }`
    );

    var str = "The `foos` with `baz` = 4 have IDs of ";

    for (let foo of result.foos) {
        str += foo.id += ", ";
    }

    return str;
}

export function testMutation(claytip) {
    return 3.14
}

export function log(env, message) {
    console.log(message)
    return true
}
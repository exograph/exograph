export function enterConcertMutation(operation, claytip) {
    logEntry(`enterConcertMutation: ${operation.name()}`, claytip)
    return true
}

export function enterVenueMutation(operation, claytip) {
    console.log(`exitConcertMutation: ${operation.name()}`);
    logEntry(`enterVenueMutation: ${operation.name()}`, claytip)
    return true
}

export function exitConcertMutation(operation, claytip) {
    logEntry(`exitConcertMutation ${operation.name()}`, claytip);
}

export function exitVenueMutation(operation, claytip) {
    console.log(`exitVenueMutation: ${operation.name()}`);
    logEntry(`exitVenueMutation: ${operation.name()}`, claytip)
    return true
}

export function enterQuery(operation, claytip) {
    logEntry(`enterQuery: ${operation.name()}`, claytip)
    return true
}

export function exitQuery(operation, claytip) {
    logEntry(`exitQuery: ${operation.name()}`, claytip)
    return true
}

export function enterMutation(operation, claytip) {
    logEntry(`enterMutation: ${operation.name()}`, claytip)
    return true
}

export function rateLimitingQuery(operation, claytip) {
    logEntry(`start rateLimitingQuery: ${operation.name()}`, claytip)
    const res = operation.proceed();
    logEntry(`end rateLimitingQuery: ${operation.name()}`, claytip)

    return res;
}

export function timingQuery(operation, claytip) {
    logEntry(`start timingQuery: ${operation.name()}`, claytip)
    const startTime = performance.now();
    const res = operation.proceed()
    const endTime = performance.now();
    logEntry(`end timingQuery: ${operation.name()}`, claytip)

    console.log(`The query ${operation.name()} took ${endTime-startTime} milliseconds`)
    return res
}

function logEntry(message, claytip) {
    console.log(`${message}`)
    let variable = {
        "message": message
    };

    claytip.executeQuery(
        `mutation($message: String!) {
            createLog(data: {message: $message}) {
            id
          }
        }`,
        variable
    );

    return true
}
export function enterConcertMutation(operation, claytip) {
    logEntry(`enterConcertMutation: ${operation.name}`, claytip)
    return true
}

export function enterVenueMutation(operation, claytip) {
    logEntry(`enterVenueMutation: ${operation.name}`, claytip)
    return true
}

export function exitConcertMutation(operation, claytip) {
    logEntry(`exitConcertMutation ${operation.name}`, claytip);
}

export function exitVenueMutation(operation, claytip) {
    logEntry(`exitVenueMutation: ${operation.name}`, claytip)
    return true
}

export function enterQuery(operation, claytip) {
    logEntry(`enterQuery: ${operation.name}`, claytip)
    return true
}

export function exitQuery(operation, claytip) {
    logEntry(`exitQuery: ${operation.name}`, claytip)
    return true
}

export function enterMutation(operation, claytip) {
    logEntry(`enterMutation: ${operation.name}`, claytip)
    return true
}

export function timeQuery(operation, claytip) {
    const startTime = performance.now();
    console.log(`startTime ${startTime}`);
    operation.proceed()
    const endTime = performance.now();
    console.log(`The query ${operation.name} took ${endTime-startTime} milliseconds`)
    return true
}

function logEntry(message, claytip) {
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
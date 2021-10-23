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
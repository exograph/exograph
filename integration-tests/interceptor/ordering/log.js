export async function enterConcertMutation(operation, claytip) {
    await logEntry(`enterConcertMutation: ${operation.name()}`, claytip)
    return true
}

export async function enterVenueMutation(operation, claytip) {
    console.log(`exitConcertMutation: ${operation.name()}`);
    await logEntry(`enterVenueMutation: ${operation.name()}`, claytip)
    return true
}

export async function exitConcertMutation(operation, claytip) {
    await logEntry(`exitConcertMutation ${operation.name()}`, claytip);
}

export async function exitVenueMutation(operation, claytip) {
    console.log(`exitVenueMutation: ${operation.name()}`);
    await logEntry(`exitVenueMutation: ${operation.name()}`, claytip)
    return true
}

export async function enterQuery(operation, claytip) {
    await logEntry(`enterQuery: ${operation.name()}`, claytip)
    return true
}

export async function exitQuery(operation, claytip) {
    await logEntry(`exitQuery: ${operation.name()}`, claytip)
    return true
}

export async function enterMutation(operation, claytip) {
    await logEntry(`enterMutation: ${operation.name()}`, claytip)
    return true
}

export async function rateLimitingQuery(operation, claytip) {
    await logEntry(`start rateLimitingQuery: ${operation.name()}`, claytip)
    const res = await operation.proceed();
    await logEntry(`end rateLimitingQuery: ${operation.name()}`, claytip)

    return res;
}

export async function timingQuery(operation, claytip) {
    await logEntry(`start timingQuery: ${operation.name()}`, claytip)
    const startTime = performance.now();
    const res = await operation.proceed();
    const endTime = performance.now();
    await logEntry(`end timingQuery: ${operation.name()}`, claytip)

    console.log(`The query ${operation.name()} took ${endTime-startTime} milliseconds`)
    return res
}

async function logEntry(message, claytip) {
    console.log(`${message}`)
    let variable = {
        "message": message
    };

    await claytip.executeQuery(
        `mutation($message: String!) {
            createLog(data: {message: $message}) {
            id
          }
        }`,
        variable
    );

    return true
}
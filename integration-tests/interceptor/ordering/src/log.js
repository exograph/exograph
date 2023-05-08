export async function enterConcertMutation(operation, exograph) {
    await logEntry(`enterConcertMutation: ${operation.name()}`, exograph)
    return true
}

export async function enterVenueMutation(operation, exograph) {
    console.log(`exitConcertMutation: ${operation.name()}`);
    await logEntry(`enterVenueMutation: ${operation.name()}`, exograph)
    return true
}

export async function exitConcertMutation(operation, exograph) {
    await logEntry(`exitConcertMutation ${operation.name()}`, exograph);
}

export async function exitVenueMutation(operation, exograph) {
    console.log(`exitVenueMutation: ${operation.name()}`);
    await logEntry(`exitVenueMutation: ${operation.name()}`, exograph)
    return true
}

export async function enterQuery(operation, exograph) {
    await logEntry(`enterQuery: ${operation.name()}`, exograph)
    return true
}

export async function exitQuery(operation, exograph) {
    await logEntry(`exitQuery: ${operation.name()}`, exograph)
    return true
}

export async function enterMutation(operation, exograph) {
    await logEntry(`enterMutation: ${operation.name()}`, exograph)
    return true
}

export async function rateLimitingQuery(operation, exograph) {
    await logEntry(`start rateLimitingQuery: ${operation.name()}`, exograph)
    const res = await operation.proceed();
    await logEntry(`end rateLimitingQuery: ${operation.name()}`, exograph)

    return res;
}

export async function timingQuery(operation, exograph) {
    await logEntry(`start timingQuery: ${operation.name()}`, exograph)
    const startTime = performance.now();
    const res = await operation.proceed();
    const endTime = performance.now();
    await logEntry(`end timingQuery: ${operation.name()}`, exograph)

    console.log(`The query ${operation.name()} took ${endTime - startTime} milliseconds`)
    return res
}

async function logEntry(message, exograph) {
    console.log(`${message}`)
    let variable = {
        "message": message
    };

    await exograph.executeQuery(
        `mutation($message: String!) {
            createLog(data: {message: $message}) {
            id
          }
        }`,
        variable
    );

    return true
}
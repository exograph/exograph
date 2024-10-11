export function requestContext(requestContext) {
  return {
    apiKey: requestContext.apiKey,
    clientKey: requestContext.clientKey,
    sessionId: requestContext.sessionId,
  };
}

export function whatsMyIp(requestContext) {
  return requestContext.ip;
}

export async function addResponseHeader(name, value, exograph) {
  await exograph.addResponseHeader(name, value);

  return "ok";
}

export async function addResponseCookie(name, value, exograph) {
  await exograph.setCookie({ name, value });

  return "ok";
}


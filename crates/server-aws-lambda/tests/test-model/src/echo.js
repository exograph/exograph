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



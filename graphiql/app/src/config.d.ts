// Make sure this file matches the PlaygroundConfig config in the playground-router crate

export type PlaygroundConfig = {
  playgroundHttpPath: string;
  graphqlHttpPath: string;

  enableSchemaLiveUpdate: boolean;

  upstreamGraphQLEndpoint?: string;

  oidcUrl?: string;

  jwtSourceHeader?: string;
  jwtSourceCookie?: string;
};

// Make sure this shape matches the PlaygroundConfig in the playground-router crate

interface Window {
  exoConfig: {
    playgroundHttpPath: string;
    graphqlHttpPath: string;
    mcpHttpPath?: string;

    enableSchemaLiveUpdate: boolean;

    upstreamGraphQLEndpoint?: string;

    oidcUrl?: string;

    jwtSourceHeader?: string;
    jwtSourceCookie?: string;
  };
}

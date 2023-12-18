---
sidebar_position: 2
---

# Environment

Both [development](development/overview.md) and [production](production/exo-server.md) CLI support a few environment variables.

## HTTP Paths

- `EXO_SERVER_HOST`: The host address of the server. Defaults to `localhost` in development and `0.0.0.0` in production.
- `EXO_SERVER_PORT`: The port of the server. Defaults to `9876`.
- `EXO_PLAYGROUND_HTTP_PATH`: The path to serve the GraphQL playground. Defaults to `/playground`.
- `EXO_ENDPOINT_HTTP_PATH`: The path to serve the GraphQL endpoint. Defaults to `/graphql`.
- `EXO_CORS_DOMAINS`: A comma-separated list of domains to allow CORS requests from. Defaults to `*` in development and empty in production.

## Authentication

JWT authentication may be specified by configuring one of the following environment variables (but not both):

- `EXO_JWT_SECRET`: The secret to use for signing JWT tokens. Defaults to a generated in "yolo" mode.
- `EXO_OIDC_URL`: The URL of the OIDC provider. For example, `https://<your-clerk-host>.clerk.accounts.dev`, `https://<your-auth0-host>.auth0.com` etc.

## Control

- `EXO_INTROSPECTION`: Whether to enable introspection. Defaults to `true` in development and `false` in production.
- `EXO_MAX_SELECTION_DEPTH`: The maximum allowed selection depth of a GraphQL query. Defaults to `15`.

## Logging

- `EXO_LOG`: The log level. Defaults to `info`. See [Telemetry](/production/telemetry.md) for more information.

Besides these standard environment variables, each plugin supports configuration through additional environment variables. Please refer to each plugin's documentation for more information. Specifically for Postgres, see [its documentation](/postgres/configuration.md).

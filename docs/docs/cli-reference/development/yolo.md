---
sidebar_position: 20
---

# exo yolo

During development, especially at the beginning, you want a low ceremony experience while developing your model. Yolo mode is a shortcut that gets you started quickly. As long as you have either Postgres or [Docker](https://docker.com) installed, in this mode, Exograph will create a temporary database with the proper schema, create a JWT secret, and start the server. As you change the model, it will automatically apply the migrations and restart the server.

In this mode, Exograph will delete any database created in the yolo mode when you stop the server. Once you have a reasonably stable model, you should switch to the [dev mode](./dev.md) to have more control over the database and retain data between restarts.

## Usage

You invoke the yolo mode using the `yolo` command in your Exograph project's directory.

```shell-session
# shell-command-next-line
exo yolo
```

By default, it will start the server on port 9876. You can change this by passing the `--port` (or the shorter `-p`) option.

By default, it will enforce [trusted documents](../../production/trusted-documents.md). You can turn this off by passing the `--enforce-trusted-documents=false` option.

```shell-session
# shell-command-next-line
exo yolo --port 8888
```

It will then print the information necessary to connect to the server.

```
Launching PostgreSQL locally...
Watching the src directory for changes...
Starting with a temporary database (will be wiped out when the server exits)...
Postgres URL: postgres://exo@%2Fvar%2Ffolders%2F8g%2Fttrcklpj7879w6fbk26dgrbh0000gn%2FT%2F.tmpcYt5yp/yolo
Generated JWT secret: c1d22ndtjjxlxni
Applying migrations...
Started server on localhost:9876 in 6.14 ms
- GraphQL hosted at:
        http://localhost:9876/graphql
- MCP endpoint hosted at:
        http://localhost:9876/mcp        
- Playground hosted at:
        http://localhost:9876/playground
```

import EphemeralDb from './_ephemeral_db.md';

<EphemeralDb/>

## Authentication Options

By default, the yolo mode will use symmetric authentication with an auto-generated secret. However, you may use your secret or an external OpenID provider for authentication.

### Overriding Auto-generated Secret

You may specify the `EXO_JWT_SECRET` environment variable to override an auto-generated one. This helps during debugging to keep the secret stable across multiple invocations of `exo yolo`.

```shell-session
# shell-command-next-line
EXO_JWT_SECRET=secret exo yolo
```

The output will indicate that your secret is in effect.

```
...
JWT secret: Using the EXO_JWT_SECRET env value
...
```

You may use the JWT secret to create a JWT token for testing authentication. The easiest way to do so is through Exograph's playground. For more details, please see [symmetric authentication in playground](/authentication/playground/symmetric.md).

### OpenID Connect

To use OpenID Connect for authentication, specify the OpenID Connect server's URL using the `EXO_OIDC_URL` environment variable.

```shell-session
# shell-command-next-line
EXO_OIDC_URL=https://<your-authentication-provider-url> exo yolo
```

The output will indicate that you are using OpenID Connect.

```
...
OIDC URL: Using the EXO_OIDC_URL env value
...
```

For more details, please see [OpenID Connect](/authentication/configuration.md#openid-connect).

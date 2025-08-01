---
sidebar_position: 75
---

# Managing Environments

Exograph provides flexible environment variable management through `.env*` files and environment modes. You can configure different settings for development, testing, and production environments.

## Environment Modes

The `EXO_ENV` environment variable controls loading of `.env*` files. It also controls the [IP address of the server](cli-reference/environment.md#http-paths).

For `exo yolo`, `exo dev`, `exo test`, and `exo playground`, Exograph automatically sets this variable to the subcommand name. For example, when running `exo yolo`, `EXO_ENV` is set to `yolo`.

You can also set it manually to control the environment mode when running `exo-server` or other `exo` commands such as `exo schema migrate`. For example, when running `EXO_ENV=production exo-server`, Exograph will files with the `production` mode as described [below](#environment-file-loading). Similarly, when running `EXO_ENV=dev exo schema migrate`, Exograph will files with the `dev` mode.

## Environment File Loading

Exograph loads environment variables from `.env` files based on the environment mode. The loading order follows this precedence (highest to lowest):

1. System environment variables
2. `.env.{mode}.local` (e.g., `.env.dev.local`)
3. `.env.local`
4. `.env.{mode}` (e.g., `.env.dev`)
5. `.env`

Higher precedence files override variables in lower precedence files. For example, if `.env.dev.local` contains `EXO_INTROSPECTION=true` and `.env.dev` contains `EXO_INTROSPECTION=false`, Exograph will use `EXO_INTROSPECTION=true` from the `.env.dev.local` file.

## Environment File Types

### Local Files (`.local` suffix)

Use these files with specific environment variables for local development. Never commit them to version control. For example, you can create a `.env.dev.local` file with the following content to point to a local database and a JWT secret for development:

```properties
DATABASE_URL=postgres://localhost/finance-dev
EXO_JWT_SECRET=your-dev-secret
```

### Shared Files

If you want to share the same environment variables across multiple modes, you can create a `.env` or `.env.{mode}` file. You may commit these files to version control for shared configuration. For example, you can create a `.env.dev` file with the following content to use a shared SMTP server for development:


```properties
MAIL_HOST=dev.example.com
MAIL_PORT=587
MAIL_USERNAME=your-dev-username
MAIL_PASSWORD=your-dev-password
```

:::warning
Avoid committing `.env.production` as it may contain sensitive production secrets. New Exograph projects include a `.gitignore` file that excludes all .env files, with comments guiding you on which files are safe to commit.
:::

### Mode agnostic files

Create a `.env` file for environment variables common to all modes.

## Production Secrets

You should never include production secrets in your `.env` files. Instead, use facilities provided by your infrastructure provider. For example, if you're using Fly.io, you can use [Fly.io Secrets](https://fly.io/docs/reference/secrets/) to manage your secrets. See [Deploying Exograph on Fly.io](./deployment/flyio.md#set-the-jwt-secret-or-the-oidc-url) for more details.
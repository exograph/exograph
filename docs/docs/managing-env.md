# Managing Environment Variables

Exograph provides flexible environment variable management through `.env` files and environment modes. You can configure different settings for development, testing, and production environments.

## Environment Modes

The `EXO_ENV` environment variable controls Exograph's environment mode. Valid values for `EXO_ENV` are:

- `yolo`: Quick exploration with temporary database (`exo yolo` sets this automatically)
- `dev`: Development mode with persistent database (`exo dev` sets this automatically)
- `test`: Testing environment (`exo test` sets this automatically)
- `playground`: Playground mode to connect to an existing GraphQL endpoint (`exo playground` sets this automatically)
- `production`: Production deployment. **You must set this explicitly when running `exo-server`**.

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

Use these files for local development. Never commit them to version control:

- `.env.yolo.local`
- `.env.dev.local` 
- `.env.test.local`
- `.env.production.local`
- `.env.local`

Create a `.env.{mode}.local` file for environment variables specific to your local development environment.

```properties
DATABASE_URL=postgres://localhost/finance-dev
EXO_JWT_SECRET=your-dev-secret
```

If you want to share the same environment variables across multiple modes, you can create a `.env.local` file.

### Shared Files

You can commit these files to version control for shared configuration:
- `.env.yolo`
- `.env.dev`
- `.env.test`
- `.env` (base configuration)

**Note**: Avoid committing `.env.production` as it may contain sensitive production secrets.

New Exograph projects include a `.gitignore` file that excludes all .env files, with comments guiding you on which files are safe to commit.

Your team can share `.env.{mode}` files for environment variables common to all developers.

```properties
MAIL_HOST=dev.example.com
MAIL_PORT=587
MAIL_USERNAME=your-dev-username
MAIL_PASSWORD=your-dev-password
```

Create a `.env` file for environment variables common to all modes, similar to `.env.local`.

## Production Secrets

You should never include production secrets in your `.env` files. Instead, use facilities provided by your infrastructure provider. For example, if you're using Fly.io, you can use [Fly.io Secrets](https://fly.io/docs/reference/secrets/) to manage your secrets. See [Deploying Exograph on Fly.io](./deployment/flyio.md#set-the-jwt-secret-or-the-oidc-url) for more details.
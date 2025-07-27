---
sidebar_position: 8
---

# Configuration

Currently, Exograph supports one instance of Postgres as a database specified using the `EXO_POSTGRES_URL` environment variable, which must be in the standard Postgres URL format (`postgres://<username>:<password>@<host>:<port?>/<database>?<connection-config-key-value>`). If this environment variable is absent, Exograph will look for `DATABASE_URL` as a fallback (this simplifies deployment to platforms such as Fly.io, which sets the `DATABASE_URL` environment variable by default). In either case, if the `port` part of the URL is not specified, Exograph will default to `5432`.

By default, Exograph runs in read-only mode. You may enable write access by passing setting the `EXO_POSTGRES_READ_WRITE` environment variable to `true`.

You can also configure connection pooling using the following environment variables:

- `EXO_CONNECTION_POOL_SIZE` - The maximum number of connections in the pool. Defaults to `10`.
- `EXO_CHECK_CONNECTION_ON_STARTUP` - Whether to check the connection on startup. Defaults to `true`. This ensures that the connection is valid on startup. The connection will be checked on the first query if set to false.

You may use query parameters in the Postgres URL to configure SSL. For example, to set the verification mode to `verify-full` and specify the root certificate, you would use a URL such as `postgres://...?sslmode=verify-full&sslrootcert=/path/to/root/cert.pem`. Exograph supports the following query parameters:

- `ssl` - Whether to use SSL. This parameter is a quick way to specify SSL mode. If it is true, it has the same effect as setting `sslmode` to `verify-full`.
- `sslmode` - The SSL mode. The possible values are `verify-full`, `verify-ca`, `require`, `prefer`, `allow`, and `disable`. This parameter defaults to `prefer`, where SSL will be used if the server supports it.
- `sslrootcert` - The path to the root certificate (typically offered to be downloaded by the Postgres server provider). This parameter is only used if the `sslmode` is not set to `disable`.

---
sidebar_position: 7
---

# Configuration

Currently, Exograph supports one instance of Postgres as a database. You can configure the module using a few environment variables.

- `EXO_POSTGRES_URL` - The URL of the Postgres database. The URL must be in the `postgres://<username>:<password>@<host>:<port>/<database>?<connection-config-key-value>` format. If the username is not specified, the `EXO_POSTGRES_USER` env must be set. The `<password>` defaults to an empty string unless the `EXO_POSTGRES_PASSWORD` environment variable is set. The `<port>` defaults to `5432`.
- `EXO_POSTGRES_USER` - The username of the Postgres database. This is only used if the username is not specified in the URL.
- `EXO_POSTGRES_PASSWORD` - The password of the Postgres database. This is only used if the password is not specified in the URL.

Besides these environment variables, you can also configure connection pooling using the following environment variables:

- `EXO_CONNECTION_POOL_SIZE` - The maximum number of connections in the pool. Defaults to `10`.
- `EXO_CHECK_CONNECTION_ON_STARTUP` - Whether to check the connection on startup. Defaults to `true`. This ensures that the connection is valid on startup. The connection will be checked on the first query if set to false.

You may use query parameters in the Postgres URL to configure SSL. For example, to set the verification mode to `verify-full` and specify the root certificate, you would use a URL such as `postgres://...?sslmode=verify-full&sslrootcert=/path/to/root/cert.pem`. Exograph supports the following query parameters:

- `ssl` - Whether to use SSL. This parameter is a quick way to specify SSL mode. If it is true, it has the same effect as setting `sslmode` to `verify-full`.
- `sslmode` - The SSL mode. The possible values are `verify-full`, `verify-ca`, `require`, `prefer`, `allow`, and `disable`. The default is `prefer`, where SSL will be used if the server supports it.
- `sslrootcert` - The path to the root certificate (typically offered to be downloaded by the Postgres server provider). This is only used if the `sslmode` is not set to `disable`.

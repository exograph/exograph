---
sidebar_position: 5
---

# Docker

Many cloud providers support running Docker containers. Thus, Exograph can be deployed through Docker. This document provides instructions on performing specific tasks and running an Exograph server locally for development using Docker.

:::info Explicitly supported providers
Exograph explicitly supports running a server in [Fly.io](../flyio) and [Railway](../railway). While the underlying mechanism uses Docker, you should follow the specific deployment guides for these providers.

Exograph does **not** use a Docker container for AWS Lambda or Cloudflare Workers. To deploy in those environments, please follow the [AWS Lambda](../aws-lambda) or [Cloudflare Workers](../cloudflare-workers) guides.
:::

Exograph provides three Docker images:

- `ghcr.io/exograph/cli`: It includes only the [`exo` CLI](../cli-reference/development) and may be used for development tasks such as building the `exo_ir` file and computing the database schema. Note that you cannot run `exo yolo` or `exo dev` using this image (you can use `ghcr.io/exograph/dev` for this purpose).
- `ghcr.io/exograph/server`: It includes only the Exograph server and forms the basis for running it in production.
- `ghcr.io/exograph/dev`: It includes the Exograph server, the `exo` CLI, and Postgres 16. Since it is optimized for development, it is a rather large image, and it is **not recommended** for production use. Typically, you will enter [into a shell for this image to perform development tasks, including `exo yolo` and `exo dev`](../getting-started/docker).

Let's see how you can use these images for specific tasks.

## Building the `exo_ir` file

To run an Exograph server, you must build an `exo_ir` file. While you would typically build this file using the [build](../cli-reference/development/build.md) command to use the locally installed `exo` CLI, you can also build this file using a Docker container using the `ghcr.io/exograph/cli` image (you may also use `ghcr.io/exograph/dev`).

```sh
# shell-command-next-line
docker run --rm --platform linux/x86_64 \
  --mount type=bind,source="$(pwd)",target=/usr/src/app \
  -it ghcr.io/exograph/cli:latest bash -c "exo build"
```

This will build the `exo_ir` file and put it in the `target` directory.

## Running the server

To run the server locally, you will need to pass the Postgres connection URL such that the Docker container can resolve it.

```sh
# shell-command-next-line
docker run --rm --platform linux/x86_64 \
  --mount type=bind,source="$(pwd)",target=/usr/src/app \
  -e EXO_POSTGRES_URL=<postgres-url> \
  -e EXO_INTROSPECTION=true \
  -p 9876:9876 \
  -it ghcr.io/exograph/dev:latest exo-server
```

The URL form will depend on the database you are using.

- **Cloud database**: If you use a cloud Postgres database such as [Neon](https://neon.tech/), simply pass the connection URL given to you by the database provider.
- **Local database**: If you use a local Postgres database, you may use `host.docker.internal` as the host since this is a special DNS name that resolves to the host machine. That URL will be of the form `postgres://<user>:<password>@host.docker.internal:5432/<database>`.

You may pass any additional environment such as `EXO_JWT_SECRET` and `EXO_OIDC_URL` to configure authentication and `EXO_INTROSPECTION` to enable introspection, etc.

## Building a Docker image

If you need a generic (cloud-provider-agnostic) Docker image for your application, you can start with the following Dockerfile:

```dockerfile
FROM ghcr.io/exograph/cli:latest as builder

WORKDIR /app

COPY ./src ./src

# In some setups, we need to run `exo build` as root (otherwise, we get permission errors)
USER root
RUN exo build

# You may add additional build steps such as
# `RUN exo schema migrate` to run migrations automatically.

FROM ghcr.io/exograph/server:latest

WORKDIR /app

COPY --from=builder /app/target/index.exo_ir ./target/index.exo_ir

# You may pass additional environment variables to the server in the following line.

CMD ["sh", "-c", "EXO_POSTGRES_URL=$EXO_POSTGRES_URL exo-server"]
```

Notice how we use a multi-stage build to first build the `exo_ir` file using the `ghcr.io/exograph/cli` image and then copy it into the `ghcr.io/exograph/server` image.

You can then build the Docker container using:

```sh
# shell-command-next-line
docker build -t todo-api .
```

You can then run the Docker container using:

```sh
# shell-command-next-line
docker run \
  --platform linux/x86_64 \
  --name todo-api \
  -e EXO_POSTGRES_URL=<postgres-url> \
  -e EXO_SERVER_HOST=0.0.0.0 \
  -p 9876:9876 \
  -d todo-api
```

You must pass `-e EXO_SERVER_HOST=0.0.0.0` since the container needs to bind to all network interfaces.

Of course, you may pass any additional environment such as `EXO_JWT_SECRET` and `EXO_OIDC_URL` to configure authentication and `EXO_INTROSPECTION` to enable introspection, etc.

Note the use of `-d` to run the container in detached mode. It will free up the terminal for you to continue using. If you need to go into the container, you can use `docker exec -it todo-api bash` and if you need to stop it, you can use `docker stop todo-api`.

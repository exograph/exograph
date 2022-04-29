FROM rust:1.60.0-buster as builder

# Compile the binaries (clay and clay-server)

ARG BUILD_DIR
ARG BUILD_FLAG

## Install NodeJS (needed by tree-sitter and building the GraphiQL app) and tree-sitter
RUN curl -fsSL https://deb.nodesource.com/setup_16.x | bash - && apt-get install -y nodejs
RUN cargo install --version 0.20.6 tree-sitter-cli

WORKDIR /usr/src

## Build and cache the dependencies
### Create empty projects with the right type (lib/bin)
RUN USER=root cargo new --vcs none --bin payas-cli
RUN USER=root cargo new --vcs none --lib payas-deno
RUN USER=root cargo new --vcs none --lib payas-model
RUN USER=root cargo new --vcs none --lib payas-parser
RUN USER=root cargo new --vcs none --bin payas-server-actix
RUN USER=root cargo new --vcs none --lib payas-server-core
RUN USER=root cargo new --vcs none --lib payas-sql
RUN USER=root cargo new --vcs none --lib payas-test

### Copy over Cargo.toml and Cargo.lock files so that we can build just
### the dependencies and cache this layer when only source files change
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./payas-cli/Cargo.toml ./payas-cli/Cargo.toml
COPY ./payas-deno/Cargo.toml ./payas-deno/Cargo.toml
COPY ./payas-model/Cargo.toml ./payas-model/Cargo.toml
COPY ./payas-parser/Cargo.toml ./payas-parser/Cargo.toml
COPY ./payas-server-actix/Cargo.toml ./payas-server-actix/Cargo.toml
COPY ./payas-server-core/Cargo.toml ./payas-server-core/Cargo.toml
COPY ./payas-sql/Cargo.toml ./payas-sql/Cargo.toml
COPY ./payas-test/Cargo.toml ./payas-test/Cargo.toml

COPY ./graphiql/package.json ./graphiql/package.json
COPY ./graphiql/package-lock.json ./graphiql/package-lock.json

### Compile the depdencies and remove artifacts related to the non-dependency parts (so that when we use real code, they get rebuilt)
RUN cargo build ${BUILD_FLAG} && rm */src/*.rs && rm target/${BUILD_DIR}/deps/payas* && rm -rf target/${BUILD_DIR}/.fingerprint/payas* 
RUN cd graphiql && npm install

## Build the actual image
### Copy over the source files
ADD payas-cli payas-cli/
ADD payas-deno payas-deno/
ADD payas-model payas-model/
ADD payas-parser payas-parser/
ADD payas-server-actix payas-server-actix/
ADD payas-server-core payas-server-core/
ADD payas-sql payas-sql/
ADD payas-test payas-test/

ADD graphiql graphiql/

### Build the binaries (first the graphiql app and then the rest)
RUN cd graphiql && npm run prod-build
RUN cargo build ${BUILD_FLAG}

# Create an image to include the compiled binaries
# TODO: Resolve the openssl issues and then "FROM debian:buster-slim as clay-builder"
FROM rust:1.60.0-slim-buster as clay-builder

ARG BUILD_DIR
ARG APP=/usr/src/app

RUN mkdir -p ${APP}

COPY --from=builder /usr/src/target/${BUILD_DIR}/clay-server ${APP}/clay-server
COPY --from=builder /usr/src/target/${BUILD_DIR}/clay ${APP}/clay


# Create an image that can be used for development--the `clay serve` style
FROM clay-builder as clay-dev

ARG BUILD_DIR
ARG APP=/usr/src/app

RUN apt-get update \
  && apt-get install -y ca-certificates tzdata \
  && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC
ENV APP_USER=clay

RUN groupadd $APP_USER \
  && useradd -g $APP_USER $APP_USER \
  && mkdir -p ${APP}

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

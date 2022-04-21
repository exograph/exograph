FROM rust:1.60 as builder

# 1: Compile the binaries (clay and clay-server)

# Install NodeJS (needed by tree-sitter and building the GraphiQL app)
RUN curl -fsSL https://deb.nodesource.com/setup_16.x | bash - && apt-get install -y nodejs
RUN cargo install --version 0.20.6 tree-sitter-cli


WORKDIR /usr/src

# 1.1: Build and cache the dependencies
# 1.1.1: Create empty projects with the right type (lib/bin)
RUN USER=root cargo new --vcs none --bin payas-cli
RUN USER=root cargo new --vcs none --lib payas-deno
RUN USER=root cargo new --vcs none --lib payas-model
RUN USER=root cargo new --vcs none --lib payas-parser
RUN USER=root cargo new --vcs none --bin payas-server-actix
RUN USER=root cargo new --vcs none --lib payas-server-core
RUN USER=root cargo new --vcs none --lib payas-sql
RUN USER=root cargo new --vcs none --lib payas-test

## 1.1.2: Copy over Cargo.toml and Cargo.lock files so that we can build just
##        the dependencies and cache this layer when only source files change
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

## 1.1.3: Remove artifacts related to the non-dependency parts (so that when we use real code, they get rebuilt)
RUN cargo build --release && rm */src/*.rs && rm target/release/deps/payas* && rm -rf target/release/.fingerprint/payas* 

# 1.2: Build the actual image
# 1.2.1: Copy over the source files
ADD payas-cli payas-cli/
ADD payas-deno payas-deno/
ADD payas-model payas-model/
ADD payas-parser payas-parser/
ADD payas-server-actix payas-server-actix/
ADD payas-server-core payas-server-core/
ADD payas-sql payas-sql/
ADD payas-test payas-test/

COPY graphiql graphiql

# 1.2.2: Build the binaries (first the graphiql app and then the rest)
RUN cd graphiql && npm install && npm run prod-build
RUN cargo build --release

# 1.3: Create an image to include the compiled binaries

FROM debian:buster-slim

ARG APP=/usr/src/app

RUN apt-get update \
  && apt-get install -y ca-certificates tzdata \
  && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC
ENV APP_USER=payas

RUN groupadd $APP_USER \
  && useradd -g $APP_USER $APP_USER \
  && mkdir -p ${APP}

COPY --from=builder /usr/src/target/release/clay-server ${APP}/clay-server
COPY --from=builder /usr/src/target/release/clay ${APP}/clay

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}


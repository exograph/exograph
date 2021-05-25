FROM rust:1.52 as builder

WORKDIR /usr/src

# First create a project stucture with the right dependencies (see the "COPY ..Cargo.toml" portion below)
RUN USER=root cargo new --vcs none --lib payas-model
RUN USER=root cargo new --vcs none --lib payas-parser
RUN USER=root cargo new --vcs none --lib payas-sql
RUN USER=root cargo new --vcs none --bin payas-server

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./payas-model/Cargo.toml ./payas-model/Cargo.toml
COPY ./payas-parser/Cargo.toml ./payas-parser/Cargo.toml
COPY ./payas-sql/Cargo.toml ./payas-sql/Cargo.toml
COPY ./payas-server/Cargo.toml ./payas-server/Cargo.toml

# So that we can build just the dependencies and cache this layer when only source files change
# Also remove artifacts related to the non-dependency parts (so that when we use real code, they get rebuilt)
RUN cargo build --bin payas-server --release && rm */src/*.rs && rm target/release/deps/payas* && rm -rf target/release/.fingerprint/payas* 

# Now add real code
ADD payas-model payas-model/
ADD payas-parser payas-parser/
ADD payas-sql payas-sql/
ADD payas-server payas-server/

RUN cargo build --bin payas-server --release
RUN strip target/release/payas-server



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

COPY --from=builder /usr/src/target/release/payas-server ${APP}/payas-server

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}


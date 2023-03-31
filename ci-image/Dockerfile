# Exo CI

FROM       rust:buster
MAINTAINER PayaLabs <payalabs>

LABEL Description="Base CI Image for Exograph" Version="0.0.1"

# Install NodeJS
RUN curl -fsSL https://deb.nodesource.com/setup_16.x | bash - && apt-get install -y nodejs

# Install Rust tools
RUN rustup component add rustfmt

RUN rustup component add clippy

RUN cargo install sccache

ENV RUSTC_WRAPPER sccache

ENV SCCACHE_DIR /sccache

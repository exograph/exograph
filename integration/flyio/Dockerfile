FROM clay-builder:latest as builder

COPY ./integration-tests/basic-model-no-auth/concerts.clay .
RUN ./clay build concerts.clay

#2: Run clay with plain buster-slim as the base
#FROM debian:buster
FROM rust:1.60.0-buster

# 2.1 Copy clay-server from the builder image and the claypot created in the previous step
COPY --from=builder /usr/src/app/clay-server ./clay-server
COPY --from=builder /usr/src/app/concerts.claypot ./concerts.claypot

# 2.2: Bridge between the ENV Fly exposes for the database and the one clay needs 
RUN echo ${DATABASE_URL}
# ENV CLAY_DATABASE_URL=${DATABASE_URL}
COPY ./integration/flyio/clay-server-flyio.sh .

EXPOSE 8080

ENV CLAY_JWT_SECRET="abcd"
ENV CLAY_CORS_DOMAINS="*" 
ENV CLAY_CONNECTION_POOL_SIZE=10
ENV CLAY_CHECK_CONNECTION_ON_STARTUP=false 
ENV CLAY_INTROSPECTION=true

ENV CLAY_SERVER_PORT=8080

ENV APP_USER=clay
RUN groupadd $APP_USER \
  && useradd -g $APP_USER $APP_USER
USER ${APP_USER}

CMD ["./clay-server-flyio.sh", "concerts.claypot"]

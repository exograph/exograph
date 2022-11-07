FROM clay-builder:latest as builder

ARG CLAY_FILE

WORKDIR /usr/src/app

COPY ${CLAY_FILE} .
RUN ./clay build ${CLAY_FILE}



#2: Run clay with plain buster-slim as the base
#FROM debian:buster-slim
FROM rust:1.60.0-buster

ARG CLAY_FILE
ENV CLAYPOT_FILE=${CLAY_FILE}pot

# 2.1 Copy clay-server from the builder image and the claypot created in the previous step
COPY --from=builder /usr/src/app/clay-server ./clay-server
COPY --from=builder /usr/src/app/*.so ./
COPY --from=builder /usr/src/app/${CLAYPOT_FILE} ./${CLAYPOT_FILE}

# 2.2: Copy the script with a bridge between DATABASE_URL and CLAY_DATABASE_URL as well as env variables
COPY ./run-clay-fly.sh .

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

CMD ["./run-clay-fly.sh"]

FROM exo-builder:latest as builder

ARG EXO_FILE

WORKDIR /usr/src/app

COPY ${EXO_FILE} .
RUN ./exo build ${EXO_FILE}

#2: Run exo with plain buster-slim as the base
#FROM debian:buster-slim
FROM rust:1.65.0-buster

ARG EXO_FILE
ENV EXO_IR_FILE=${EXO_FILE}_ir

# 2.1 Copy exo-server from the builder image and the exo_ir created in the previous step
COPY --from=builder /usr/src/app/exo-server ./exo-server
COPY --from=builder /usr/src/app/*_resolver.so ./
COPY --from=builder /usr/src/app/${EXO_IR_FILE} ./${EXO_IR_FILE}

# 2.2: Copy the script with a bridge between POSTGRES_URL and EXO_POSTGRES_URL as well as env variables
COPY ./run-exo-fly.sh .

EXPOSE 8080

ENV EXO_JWT_SECRET="abcd"
ENV EXO_CORS_DOMAINS="*" 
ENV EXO_CONNECTION_POOL_SIZE=10
ENV EXO_CHECK_CONNECTION_ON_STARTUP=false 
ENV EXO_INTROSPECTION=true

ENV EXO_SERVER_PORT=8080

ENV APP_USER=exo
RUN groupadd $APP_USER \
  && useradd -g $APP_USER $APP_USER
USER ${APP_USER}

CMD ["./run-exo-fly.sh"]

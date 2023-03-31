FROM exo-builder:latest as builder

ARG EXO_FILE

WORKDIR /usr/src/app

# 1. Build .exo_ir and index.sql
COPY ${EXO_FILE} .
RUN ./exo build ${EXO_FILE}

# 2. Build final image
FROM rust:1.65.0-buster
WORKDIR /usr/src/app

ARG EXO_FILE
ENV EXO_IR_FILE=${EXO_FILE}_ir

# 3 Copy artifacts created in the previous steps
COPY --from=builder /usr/src/app/exo-server ./exo-server
COPY --from=builder /usr/src/app/*_resolver.so ./
COPY --from=builder /usr/src/app/${EXO_IR_FILE} ./index.exo_ir
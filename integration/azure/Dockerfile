FROM clay-builder:latest as builder

ARG CLAY_FILE

WORKDIR /usr/src/app

# 1. Build .claypot and index.sql
COPY ${CLAY_FILE} .
RUN ./clay build ${CLAY_FILE}

# 2. Build final image
FROM clay:latest

ARG CLAY_FILE
ENV CLAYPOT_FILE=${CLAY_FILE}pot

# 3 Copy artifacts created in the previous steps
COPY --from=builder /usr/src/app/clay-server ./clay-server
COPY --from=builder /usr/src/app/${CLAYPOT_FILE} ./index.claypot
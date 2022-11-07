FROM clay-builder-aws:latest as builder

ARG CLAY_FILE

WORKDIR /usr/src/app

# 1. Build .claypot and index.sql
COPY ${CLAY_FILE} .
RUN ./clay build ${CLAY_FILE}
RUN sh -c './clay schema create ${CLAY_FILE} > index.sql'

# 2. Install python dependencies for aws-cf-func
FROM public.ecr.aws/lambda/python:3.8 as python-deps-builder

RUN mkdir -p /usr/src/python-deps
ADD requirements.txt .
RUN pip3 install -r requirements.txt --target /usr/src/python-deps

# 3. Build final image
FROM clay-aws:latest

ARG CLAY_FILE
ENV CLAYPOT_FILE=${CLAY_FILE}pot

# 3.1 Copy artifacts created in the previous steps
COPY --from=builder /usr/src/app/bootstrap ./bootstrap
COPY --from=builder /usr/src/app/*.so ./
COPY --from=builder /usr/src/app/${CLAYPOT_FILE} ./index.claypot
COPY --from=builder /usr/src/app/index.sql ./index.sql
COPY --from=python-deps-builder /usr/src/python-deps ./python-deps

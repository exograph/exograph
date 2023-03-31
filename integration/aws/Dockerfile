FROM exo-builder-aws:latest as builder

ARG EXO_FILE

WORKDIR /usr/src/app

# 1. Build .exo_ir and index.sql
COPY ${EXO_FILE} .
RUN ./exo build ${EXO_FILE}
RUN sh -c './exo schema create ${EXO_FILE} > index.sql'

# 2. Install python dependencies for aws-cf-func
FROM public.ecr.aws/lambda/python:3.8 as python-deps-builder

RUN mkdir -p /usr/src/python-deps
ADD requirements.txt .
RUN pip3 install -r requirements.txt --target /usr/src/python-deps

# 3. Build final image
FROM exo-aws:latest

ARG EXO_FILE
ENV EXO_IR_FILE=${EXO_FILE}_ir

# 3.1 Copy artifacts created in the previous steps
COPY --from=builder /usr/src/app/bootstrap ./bootstrap
COPY --from=builder /usr/src/app/*_resolver.so ./
COPY --from=builder /usr/src/app/${EXO_IR_FILE} ./index.exo_ir
COPY --from=builder /usr/src/app/index.sql ./index.sql
COPY --from=python-deps-builder /usr/src/python-deps ./python-deps

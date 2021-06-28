FROM payas-server:latest

COPY ./integration-tests/basic-model-no-auth/concerts.clay .

COPY ./integration/flyio/clay-server-flyio.sh .

EXPOSE 8080
ENV CLAY_SERVER_PORT=8080
ENV CLAYS_CORS_DOMAINS=*

CMD ["./clay-server-flyio.sh", "concerts.clay"]

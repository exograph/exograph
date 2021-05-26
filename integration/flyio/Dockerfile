FROM payas-server:latest

COPY ./test-models/bb.payas .

COPY ./integration/flyio/payas-server-flyio.sh .

EXPOSE 8080
ENV PAYAS_SERVER_PORT=8080
ENV PAYAS_CORS_DOMAINS=*

CMD ["./payas-server-flyio.sh", "bb.payas"]

# syntax=docker/dockerfile:1
FROM python:3.13-slim-trixie
ARG HTTPS_PROXY
ARG HTTP_PROXY
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then cp /run/secrets/proxy_ca /usr/local/share/ca-certificates/build-proxy.crt; update-ca-certificates; fi && \
    apt-get update && apt-get install -y --no-install-recommends gcc g++ libc6-dev docker-cli ca-certificates curl && \
    curl -fsS https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem -o /etc/ssl/certs/aws-rds.pem && \
    rm -rf /var/lib/apt/lists/* /usr/local/share/ca-certificates/build-proxy.crt && update-ca-certificates --fresh
COPY requirements.txt /app/requirements.txt
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then PIP_CERT=/run/secrets/proxy_ca pip install --no-cache-dir -r /app/requirements.txt; else pip install --no-cache-dir -r /app/requirements.txt; fi
COPY .build/traojudge-judge /usr/local/bin/traojudge-judge
COPY worker.py /app/worker.py
# Child sandboxes override this command and receive neither IMDS networking nor the Docker socket.
CMD ["python3", "/app/worker.py"]

COPY examples.py /app/examples.py

RUN docker --version && cc --version >/dev/null && python3 -c "import boto3"

FROM python:3.13-slim-trixie@sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb
RUN --mount=type=secret,id=proxy_ca,required=false,mode=0444 \
    sed -i 's|http://deb.debian.org|https://deb.debian.org|g' /etc/apt/sources.list.d/debian.sources && \
    if [ -f /run/secrets/proxy_ca ]; then \
      apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca update && \
      apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca install -y --no-install-recommends gcc libc6-dev libseccomp-dev; \
    else apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libseccomp-dev; fi && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY fixture.c sandbox.c server.py /app/
RUN chmod 644 fixture.c sandbox.c server.py && \
    gcc -O2 -Wall -Wextra -Werror sandbox.c -lseccomp -o /app/sandbox && \
    gcc -O2 -std=c17 -fno-tree-vectorize -march=armv8-a fixture.c -o /app/fixture
EXPOSE 8080
CMD ["python3", "/app/server.py"]

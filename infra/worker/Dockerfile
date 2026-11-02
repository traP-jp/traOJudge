FROM python:3.13-slim-trixie
RUN --mount=type=secret,id=proxy_ca,required=true,mode=0444 \
    sed -i 's|http://deb.debian.org|https://deb.debian.org|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca update && \
    apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca install -y --no-install-recommends gcc libc6-dev libseccomp-dev && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /opt
COPY --chmod=644 sandbox.c check.py summarize.py worker.py /opt/
RUN gcc -O2 -Wall -Wextra -Werror sandbox.c -lseccomp -o sandbox
ENTRYPOINT ["python3", "/opt/worker.py"]

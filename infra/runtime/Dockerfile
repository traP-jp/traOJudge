FROM python:3.13-slim-trixie
RUN --mount=type=secret,id=proxy_ca,required=true \
    PIP_CERT=/run/secrets/proxy_ca pip install --no-cache-dir boto3==1.42.20
WORKDIR /opt
COPY --chmod=644 *.py /opt/
COPY --chmod=755 engine /opt/engine
ENTRYPOINT ["python3", "/opt/runtime.py"]

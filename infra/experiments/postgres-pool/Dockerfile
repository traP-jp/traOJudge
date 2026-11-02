ARG BASE=nixos/nix@sha256:e623d73af9cac82d1b50784c83e0cf2a4b83bfd2cfe8d5b67809a2fc94e043ac
FROM ${BASE}
COPY . /opt/
RUN --mount=type=secret,id=proxy_ca \
    set -eu; if test -f /run/secrets/proxy_ca; then export NIX_SSL_CERT_FILE=/run/secrets/proxy_ca; fi; \
    for lang in cpp python rust; do \
      nix --extra-experimental-features 'nix-command flakes' build --option sandbox false \
        --no-update-lock-file --out-link /opt/$lang path:/opt/baked-command#runtime-$lang || exit 1; \
    done; \
    nix --extra-experimental-features 'nix-command flakes' build --option sandbox false \
      --no-update-lock-file --out-link /opt/tools path:/opt/baked-command#probe-tools; \
    chmod 755 /opt/judge-probe
ENV PATH=/opt/tools/bin:/nix/var/nix/profiles/default/bin:/root/.nix-profile/bin:/bin
ENV JUDGE_FLAKE=/opt/baked-command
CMD ["sleep", "infinity"]

FROM scratch
COPY guest-root-marker /traojudge
LABEL org.opencontainers.image.description="Empty guest root; only a selected Nix closure is mounted read-only"

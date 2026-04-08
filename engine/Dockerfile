###############################################################################
# Scutar engine — multi-stage, multi-arch (amd64 + arm64) Dockerfile
#
# Build:   docker buildx build --platform linux/amd64,linux/arm64 \
#              -t scutar/scutar-runner:latest engine/ --push
###############################################################################

ARG RUST_VERSION=1.82

# ---- builder ----------------------------------------------------------------
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /build

# System deps for the SDK / SSH crates (openssl, ssh, ca-certs).
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates cmake \
 && rm -rf /var/lib/apt/lists/*

# Cache dependencies separately from sources for fast rebuilds.
COPY Cargo.toml ./
COPY crates ./crates

RUN cargo build --release --bin scutar-runner

# ---- runtime ----------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 65532 --user-group --create-home --home-dir /home/scutar scutar

COPY --from=builder /build/target/release/scutar-runner /usr/local/bin/scutar-runner

USER 65532:65532
WORKDIR /home/scutar

# The operator mounts the BackupSpec as a ConfigMap at this path.
ENV SCUTAR_SPEC_PATH=/etc/scutar/spec.yaml \
    RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/scutar-runner"]
CMD ["--spec", "/etc/scutar/spec.yaml"]

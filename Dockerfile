# syntax=docker/dockerfile:1
# ------------------------------------------------------------------------------
# Stage 1: Build the headless release binary
# ------------------------------------------------------------------------------
FROM rust:1.82-bookworm AS builder

WORKDIR /usr/src/dvd-ripper

# Copy manifests first to optimize Docker layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source tree to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    mkdir src/bin && \
    echo "fn main() {}" > src/bin/installer.rs && \
    cargo build --release --no-default-features --bin dvd-ripper && \
    rm -rf src

# Copy real sources and compile final binary
COPY . .
RUN touch src/main.rs && cargo build --release --no-default-features --bin dvd-ripper

# ------------------------------------------------------------------------------
# Stage 2: Minimal appliance runtime image
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim

LABEL org.opencontainers.image.title="dvd-ripper" \
      org.opencontainers.image.description="Automated headless DVD ripper daemon & transcoding appliance" \
      org.opencontainers.image.source="https://github.com/clockworkengineer/dvd-ripper" \
      org.opencontainers.image.licenses="MIT"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    libdvd-pkg \
    ca-certificates \
    udev \
    procps \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/dvd-ripper/target/release/dvd-ripper /usr/local/bin/dvd-ripper

RUN mkdir -p /output /config

VOLUME ["/output", "/config"]

# Default environment options
ENV DVD_INPUT=/dev/sr0 \
    DVD_OUTPUT=/output \
    RUST_LOG=info

ENTRYPOINT ["dvd-ripper"]
CMD ["--daemon", "--input", "/dev/sr0", "--out-dir", "/output"]

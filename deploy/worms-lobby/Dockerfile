# Built by Coolify (same host as Hotpot).
# Coolify settings:
#   Base Directory: /
#   Dockerfile: deploy/worms-lobby/Dockerfile
#   Port: 8080
#   Ports mappings: 7800-7899 (TCP + UDP) for dedicated rooms
FROM rust:1.90-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libasound2-dev libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY games ./games
RUN cargo build -p worms-lobby --release -j 2 \
    && cargo build -p worms --bin worms-dedicated --release -j 2

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libasound2 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/worms-lobby /app/bin/worms-lobby
COPY --from=builder /src/target/release/worms-dedicated /app/bin/worms-dedicated
COPY games/worms/assets /app/assets

ENV DEDICATED_BIN=/app/bin/worms-dedicated \
    ASSETS_DIR=/app/assets \
    PORT_MIN=7800 \
    PORT_MAX=7899 \
    LOBBY_PUBLIC_HTTP=http://127.0.0.1:8080

# Coolify sets PORT; map it onto the lobby bind address.
ENV PORT=8080
EXPOSE 8080 7800-7899/tcp 7800-7899/udp
CMD ["sh", "-c", "export LOBBY_BIND=0.0.0.0:${PORT:-8080}; exec /app/bin/worms-lobby"]

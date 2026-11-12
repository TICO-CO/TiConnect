# syntax=docker/dockerfile:1

FROM rust:1.95-bookworm AS builder

WORKDIR /src
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --bin ticonnectd --bin ticonnect-cli \
    && mkdir -p /out \
    && cp target/debug/ticonnectd target/debug/ticonnect-cli /out/

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends dbus iproute2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/ticonnectd /out/ticonnect-cli /usr/local/bin/
COPY tests/lan/entrypoint.sh /usr/local/bin/ticonnect-lan-entrypoint
RUN chmod +x /usr/local/bin/ticonnect-lan-entrypoint

ENTRYPOINT ["/usr/local/bin/ticonnect-lan-entrypoint"]

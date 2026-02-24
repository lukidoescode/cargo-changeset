FROM rust:1.85.0-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    libssh2-dev \
    libssh2-static \
    zlib-dev \
    zlib-static \
    cmake \
    make \
    gcc \
    g++ \
    perl \
    linux-headers

ENV OPENSSL_STATIC=1

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release --package cargo-changeset

FROM alpine:3.21

RUN apk add --no-cache git

COPY --from=builder /build/target/release/cargo-changeset /usr/local/bin/cargo-changeset
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

LABEL org.opencontainers.image.source="https://github.com/lukidoescode/cargo-changeset"
LABEL org.opencontainers.image.description="Structured release management for Rust workspaces"
LABEL org.opencontainers.image.licenses="MIT"

ENTRYPOINT ["/entrypoint.sh"]

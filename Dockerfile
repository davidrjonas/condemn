FROM rust:1-slim-stretch as build

RUN apt-get update \
 && apt-get install -y build-essential pkg-config libssl-dev curl \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.* ./
RUN cargo fetch

COPY src/ ./src/
RUN cargo build --release

FROM debian:stretch-slim

RUN apt-get update \
 && apt-get install -y openssl ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/target/release/condemn /usr/local/bin/

ENTRYPOINT ["condemn"]

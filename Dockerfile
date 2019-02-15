FROM rust:1-slim-stretch as build

RUN apt-get update \
 && apt-get install -y pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo install --path .

FROM debian:stretch-slim

RUN apt-get update \
 && apt-get install -y openssl \
 && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/target/release/condemn /usr/local/bin/

ENTRYPOINT ["condemn"]

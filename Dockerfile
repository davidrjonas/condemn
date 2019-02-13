FROM rust:1-slim-stretch as build

WORKDIR /build
COPY . .

RUN cargo install --path .

FROM debian:stretch-slim

COPY --from=build /build/target/release/condemn /usr/local/bin/

ENTRYPOINT ["condemn"]

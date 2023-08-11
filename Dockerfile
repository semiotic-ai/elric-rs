FROM rust:1.71.0-slim as builder 

WORKDIR /app

RUN apt-get update && apt-get install -y libssl-dev musl-tools openssl perl make g++ # musl-dev musl g++
# RUN rustup target add x86_64-unknown-linux-musl

COPY . /app
RUN \
  --mount=type=cache,target=/app/target,rw \
  --mount=type=cache,target=/usr/local/cargo/registry,rw \
  cargo build  --release && \
  cp /app/target/release/elric-rs /app/elric-rs


FROM alpine:3.17.3 as app

RUN mkdir /app

RUN apk add libc6-compat

COPY --from=builder /app/elric-rs /app/elric-rs

WORKDIR /app

ENTRYPOINT ["./elric-rs"]
EXPOSE 3000
ENV RUST_LOG="info"
ENV PORT="3000"

FROM rust:1.71.0-alpine as app

WORKDIR /app

RUN apk add openssl-dev musl-dev musl g++
COPY . /app

RUN \
  --mount=type=cache,target=/app/target,rw \
  --mount=type=cache,target=/usr/local/cargo/registry,rw \
  cargo build --release && \
  cp /app/target/release/elric-rs /app/elric-rs


FROM alpine:3.17.3

RUN mkdir /app

RUN apk add libc6-compat

COPY --from=app /app/elric-rs /app/elric-rs

WORKDIR /app

ENTRYPOINT ["./elric-rs"]
EXPOSE 3000
ENV RUST_LOG="info"
ENV PORT="3000"

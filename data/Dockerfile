FROM debian:stable-slim

# install dependencies
RUN apt-get update && apt-get --no-install-recommends install -y \
    build-essential git libssl-dev pkg-config clang jq curl rustup
RUN rustup default stable

RUN addgroup -S nonroot && adduser -S nonroot -G nonroot
USER nonroot

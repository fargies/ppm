FROM debian:stable-slim

# install dependencies
RUN apt-get update && apt-get install -y \
    build-essential git libssl-dev pkg-config clang jq curl

RUN curl --fail --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup.sh && \
    sh rustup.sh -y --default-toolchain stable && \
    . "$HOME/.cargo/env" \
    ln -s $(command -v cargo) /usr/bin/

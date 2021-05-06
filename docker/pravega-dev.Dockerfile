#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

ARG FROM_IMAGE

FROM ${FROM_IMAGE} as builder-base

# Install Rust compiler.
# Based on:
#   - https://github.com/rust-lang/docker-rust-nightly/blob/master/buster/Dockerfile
#   - https://hub.docker.com/layers/rust/library/rust/1.49.0/images/sha256-71e239392f5a70bc034522a089175bd36d1344205625047ed42722a205b683b2?context=explore

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.51.0

RUN set -eux; \
    rustArch="x86_64-unknown-linux-gnu"; \
    url="https://static.rust-lang.org/rustup/archive/1.23.1/${rustArch}/rustup-init"; \
    wget --quiet "$url"; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# Build GStreamer Pravega libraries and applications.

ARG RUST_JOBS=1

WORKDIR /usr/src/gstreamer-pravega

FROM builder-base as planner
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM builder-base as cacher
RUN cargo install cargo-chef
COPY --from=planner /usr/src/gstreamer-pravega/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM builder-base as final

COPY . .

# Copy over the cached dependencies
COPY --from=cacher /usr/src/gstreamer-pravega/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}

## Build pravega-video-server

RUN cargo install --locked --jobs ${RUST_JOBS} --path pravega-video-server

## Build misc. Rust apps

RUN cargo install --locked --jobs ${RUST_JOBS} --path apps --bin \
      rtsp-camera-simulator

## Install Python apps
RUN mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/

COPY python_apps python_apps

ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH

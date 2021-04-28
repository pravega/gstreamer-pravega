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

FROM ${FROM_IMAGE}

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

## Build gst-plugin-pravega

COPY gst-plugin-pravega gst-plugin-pravega
COPY pravega-client-rust pravega-client-rust
COPY pravega-video pravega-video

RUN cd gst-plugin-pravega && \
    cargo build --locked --release --jobs ${RUST_JOBS} && \
    mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/

## Build pravega-video-server

COPY pravega-video-server pravega-video-server

RUN cd pravega-video-server && \
    cargo install --locked --jobs ${RUST_JOBS} --path .

## Build misc. Rust apps

COPY apps apps

RUN cd apps && \
    cargo install --locked --jobs ${RUST_JOBS} --path . --bin \
      rtsp-camera-simulator

## Install Python apps

COPY python_apps python_apps

ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH

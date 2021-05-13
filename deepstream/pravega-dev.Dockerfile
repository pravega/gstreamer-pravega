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

# Install Python Bindings for DeepStream.

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        less \
        python3-dev \
        python3-gi \
        python3-gst-1.0 \
        wget

RUN cd /opt/nvidia/deepstream/deepstream/lib && \
    python3 setup.py install && \
    cd /opt/nvidia/deepstream/deepstream/sources && \
    git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps

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
## Build gst-plugin-pravega.

# Copy over the cached dependencies
COPY --from=cacher /usr/src/gstreamer-pravega/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
COPY Cargo.toml .
COPY Cargo.lock .
COPY apps apps
COPY gst-plugin-pravega gst-plugin-pravega
COPY integration-test integration-test
COPY deepstream deepstream
COPY pravega-video pravega-video
COPY pravega-video-server pravega-video-server

RUN cargo build --package gst-plugin-pravega --release && \
    mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
RUN cargo install --path pravega-video-server
COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter

RUN cargo build --release --package pravega_protocol_adapter && \
    mv -v target/release/*.so /opt/nvidia/deepstream/deepstream/lib/

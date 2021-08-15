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

# Build DeepStream image with Python bindings and Rust compiler.

FROM ${FROM_IMAGE} as builder-base

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

# Install Python Bindings for DeepStream.

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        less \
        nano \
        python3-dev \
        python3-gi \
        python3-gst-1.0 \
        python3-pip \
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
    RUST_VERSION=1.54.0

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

WORKDIR /usr/src/gstreamer-pravega


# Install Cargo Chef build tool.
FROM builder-base as chef-base
ARG RUST_JOBS=1
RUN cargo install cargo-chef --jobs ${RUST_JOBS} --version 0.1.22 --locked


# Create Cargo Chef recipe.
FROM chef-base as planner
COPY Cargo.toml .
COPY Cargo.lock .
COPY apps apps
COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter
COPY gst-plugin-pravega gst-plugin-pravega
COPY integration-test integration-test
COPY pravega-video pravega-video
COPY pravega-video-server pravega-video-server
RUN cargo chef prepare --recipe-path recipe.json


# Download and build Rust dependencies for gstreamer-pravega.
FROM chef-base as cacher
COPY --from=planner /usr/src/gstreamer-pravega/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json | cat -


# Build GStreamer Pravega libraries and applications.
FROM builder-base as final

ARG RUST_JOBS=1

# Copy over the cached dependencies.
COPY --from=cacher /usr/src/gstreamer-pravega/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY Cargo.toml .
COPY Cargo.lock .
COPY apps apps
COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter
COPY gst-plugin-pravega gst-plugin-pravega
COPY integration-test integration-test
COPY pravega-video pravega-video
COPY pravega-video-server pravega-video-server

# Build gst-plugin-pravega.
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS} && \
    mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/

# Build pravega_protocol_adapter.
RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS} && \
    mv -v target/release/*.so /opt/nvidia/deepstream/deepstream/lib/

# Build misc. Rust apps.
RUN cargo install --locked --jobs ${RUST_JOBS} --path apps --bin \
        rtsp-camera-simulator
RUN cargo install --locked --jobs ${RUST_JOBS} --path integration-test --bin \
        longevity-test

# Install dependencies for applications.
RUN pip3 install \
        configargparse

# Copy applications.
COPY deepstream deepstream
COPY python_apps python_apps
ENV PYTHONPATH=/usr/src/gstreamer-pravega/python_apps/lib

# Define default entrypoint.
COPY docker/entrypoint.sh /entrypoint.sh
CMD ["/entrypoint.sh"]

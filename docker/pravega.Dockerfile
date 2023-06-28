#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

ARG DOCKER_REPOSITORY=""
ARG FROM_IMAGE

# Build image that that will contain the GStreamer source code.
FROM "${DOCKER_REPOSITORY}${FROM_IMAGE}" as gstreamer-source-code

COPY docker/build-gstreamer/install-dependencies /
RUN ["/install-dependencies"]

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

# Build image with Rust compiler.
FROM gstreamer-source-code as builder-base

# Install Rust compiler.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.63.0
RUN set -eux; \
    rustArch="x86_64-unknown-linux-gnu"; \
    url="https://static.rust-lang.org/rustup/archive/1.25.1/${rustArch}/rustup-init"; \
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
RUN cargo install cargo-chef --jobs ${RUST_JOBS} --version 0.1.51 --locked

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
RUN cargo chef cook --release --recipe-path recipe.json


# Build GStreamer Pravega libraries and applications.
FROM builder-base as pravega-dev

ARG RUST_JOBS=1

## Copy over the cached dependencies.
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
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
RUN mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
ENV GST_PLUGIN_PATH /usr/lib/x86_64-linux-gnu/gstreamer-1.0/

## Build pravega-video-server.
RUN cargo install --locked --jobs ${RUST_JOBS} --path pravega-video-server
COPY pravega-video-server/resources /opt/pravega-video-server/resources
ENV PRAVEGA_VIDEO_SERVER_RESOURCE_DIR=/opt/pravega-video-server/resources

## Build misc. Rust apps.
RUN cargo install --locked --jobs ${RUST_JOBS} --path apps --bin \
      rtsp-camera-simulator
RUN cargo install --locked --jobs ${RUST_JOBS} --path integration-test --bin \
      longevity-test

## Install Python apps.
COPY python_apps python_apps
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH


# Build base production image including OS dependencies.
FROM "${DOCKER_REPOSITORY}${FROM_IMAGE}" as prod-base
COPY docker/install-prod-dependencies /
RUN ["/install-prod-dependencies"]
RUN useradd -ms /bin/bash gstreamer-pravega
USER gstreamer-pravega
ENV GST_PLUGIN_PATH /usr/lib/x86_64-linux-gnu/gstreamer-1.0/


# Build production image with debug symbols.
FROM prod-base as debug-prod
COPY --from=debug-prod-compile /compiled-binaries /
COPY --from=pravega-dev /usr/src/gstreamer-pravega/python_apps /usr/src/gstreamer-pravega/python_apps
ENV PYTHONPATH=/usr/src/gstreamer-pravega/python_apps/lib
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH
COPY --from=pravega-dev /usr/lib/x86_64-linux-gnu/gstreamer-1.0/ /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/rtsp-camera-simulator /usr/local/bin/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/pravega-video-server /usr/local/bin/
COPY pravega-video-server/resources /opt/pravega-video-server/resources
ENV PRAVEGA_VIDEO_SERVER_RESOURCE_DIR=/opt/pravega-video-server/resources
COPY docker/entrypoint.sh /entrypoint.sh
CMD ["/entrypoint.sh"]

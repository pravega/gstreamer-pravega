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


# Build image that that will contain the GStreamer source code.
FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as gstreamer-source-code

COPY docker/build-gstreamer/install-dependencies /

RUN ["/install-dependencies"]

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

ARG GSTREAMER_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gstreamer.git
ARG GSTREAMER_CHECKOUT=master

ARG GST_PLUGINS_BASE_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-base.git
ARG GST_PLUGINS_BASE_CHECKOUT=master

ARG GST_PLUGINS_BAD_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-bad.git
ARG GST_PLUGINS_BAD_CHECKOUT=master

ARG GST_PLUGINS_GOOD_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-good.git
ARG GST_PLUGINS_GOOD_CHECKOUT=master

ARG GST_PLUGINS_UGLY_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-ugly.git
ARG GST_PLUGINS_UGLY_CHECKOUT=master

ARG GST_LIBAV_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-libav.git
ARG GST_LIBAV_CHECKOUT=master

ARG GST_RTSP_SERVER_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-rtsp-server.git
ARG GST_RTSP_SERVER_CHECKOUT=master

ARG LIBNICE_REPOSITORY=https://gitlab.freedesktop.org/libnice/libnice.git
ARG LIBNICE_CHECKOUT=2b38ba23b726694293de53c90b59b28ca11746ab

ADD docker/build-gstreamer/download /

RUN ["/download"]

ADD docker/build-gstreamer/compile /


# Compile GStreamer with debug symbols.
FROM gstreamer-source-code as debug-prod-compile
ENV DEBUG=true
ENV OPTIMIZATIONS=true
RUN ["/compile"]


# Build image with Rust compiler.
FROM debug-prod-compile as builder-base

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

WORKDIR /usr/src/gstreamer-pravega


# Install Cargo Chef build tool.
FROM builder-base as chef-base
ARG RUST_JOBS=1
RUN cargo install cargo-chef --jobs ${RUST_JOBS}


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
FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as prod-base
COPY docker/install-prod-dependencies /
RUN ["/install-prod-dependencies"]
ENV GST_PLUGIN_PATH /usr/lib/x86_64-linux-gnu/gstreamer-1.0/


# Build production image with debug symbols.
FROM prod-base as debug-prod
COPY --from=debug-prod-compile /compiled-binaries /
COPY --from=pravega-dev /usr/src/gstreamer-pravega/python_apps /usr/src/gstreamer-pravega/python_apps
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH
COPY --from=pravega-dev /usr/lib/x86_64-linux-gnu/gstreamer-1.0/ /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/pravega-video-server /usr/local/bin/
COPY pravega-video-server/resources /opt/pravega-video-server/resources
ENV PRAVEGA_VIDEO_SERVER_RESOURCE_DIR=/opt/pravega-video-server/resources


# Build GStreamer without debug symbols.
FROM gstreamer-source-code as prod-compile
ENV DEBUG=false
ENV OPTIMIZATIONS=true
RUN ["/compile"]


# Build production image without debug symbols
FROM prod-base as prod
COPY --from=prod-compile /compiled-binaries /
COPY --from=pravega-dev /usr/src/gstreamer-pravega/python_apps /usr/src/gstreamer-pravega/python_apps
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH
COPY --from=pravega-dev /usr/lib/x86_64-linux-gnu/gstreamer-1.0/ /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/pravega-video-server /usr/local/bin/
COPY pravega-video-server/resources /opt/pravega-video-server/resources
ENV PRAVEGA_VIDEO_SERVER_RESOURCE_DIR=/opt/pravega-video-server/resources

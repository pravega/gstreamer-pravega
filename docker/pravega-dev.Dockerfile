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

FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as gstreamer-source-code

ARG RUST_JOBS=1

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

FROM gstreamer-source-code as base
ENV DEBUG=true
ENV OPTIMIZATIONS=true
# Compile binaries with debug symbols and keep source code
RUN ["/compile"]

FROM base as builder-base
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

FROM builder-base as chef-base
RUN cargo install cargo-chef --jobs ${RUST_JOBS}

FROM chef-base as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef-base as cacher
COPY --from=planner /usr/src/gstreamer-pravega/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM builder-base as pravega-dev

# Copy over the cached dependencies
COPY --from=cacher /usr/src/gstreamer-pravega/target target
COPY --from=cacher /usr/local/cargo /usr/local/Cargo

COPY Cargo.toml .
COPY Cargo.lock .
COPY apps apps
COPY gst-plugin-pravega gst-plugin-pravega
COPY integration-test integration-test
COPY deepstream deepstream
COPY pravega-video pravega-video
COPY pravega-video-server pravega-video-server

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

FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as prod-base
RUN \
    apt-get update && \
    apt-get dist-upgrade -y && \
    apt-get install -y --no-install-recommends \
        bubblewrap \
        ca-certificates \
        intel-media-va-driver-non-free \
        iso-codes \
        ladspa-sdk \
        liba52-0.7.4 \
        libaa1 \
        libaom0 \
        libass9 \
        libavcodec58 \
        libavfilter7 \
        libavformat58 \
        libavutil56 \
        libbs2b0 \
        libbz2-1.0 \
        libcaca0 \
        libcap2 \
        libchromaprint1 \
        libcurl3-gnutls \
        libdca0 \
        libde265-0 \
        libdv4 \
        libdvdnav4 \
        libdvdread8 \
        libdw1 \
        libegl1 \
        libepoxy0 \
        libfaac0 \
        libfaad2 \
        libfdk-aac2 \
        libflite1 \
        libfluidsynth2 \
        libgbm1 \
        libgcrypt20 \
        libgl1 \
        libgles1 \
        libgles2 \
        libglib2.0-0 \
        libgme0 \
        libgmp10 \
        libgsl25 \
        libgsm1 \
        libgudev-1.0-0 \
        libharfbuzz-icu0 \
        libjpeg8 \
        libkate1 \
        liblcms2-2 \
        liblilv-0-0 \
        libmfx1 \
        libmjpegutils-2.1-0 \
        libmodplug1 \
        libmp3lame0 \
        libmpcdec6 \
        libmpeg2-4 \
        libmpg123-0 \
        libofa0 \
        libogg0 \
        libopencore-amrnb0 \
        libopencore-amrwb0 \
        libopenexr25 \
        libopenjp2-7 \
        libopus0 \
        liborc-0.4-0 \
        libpango-1.0-0 \
        libpng16-16 \
        librsvg2-2 \
        librtmp1 \
        libsbc1 \
        libseccomp2 \
        libshout3 \
        libsndfile1 \
        libsoundtouch1 \
        libsoup2.4-1 \
        libspandsp2 \
        libspeex1 \
        libsrt1 \
        libsrtp2-1 \
        libssl1.1 \
        libtag1v5 \
        libtheora0 \
        libtwolame0 \
        libunwind8 \
        libva2 \
        libvisual-0.4-0 \
        libvo-aacenc0 \
        libvo-amrwbenc0 \
        libvorbis0a \
        libvpx6 \
        libvulkan1 \
        libwavpack1 \
        libwayland-client0 \
        libwayland-egl1 \
        libwayland-server0 \
        libwebp6 \
        libwebpdemux2 \
        libwebpmux3 \
        libwebrtc-audio-processing1 \
        libwildmidi2 \
        libwoff1 \
        libx264-160 \
        libx265-192 \
        libxkbcommon0 \
        libxslt1.1 \
        libzbar0 \
        libzvbi0 \
        mjpegtools \
        wayland-protocols \
        xdg-dbus-proxy \
    && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

FROM gstreamer-source-code as debug-prod-compile
ENV DEBUG=true
ENV OPTIMIZATIONS=true
RUN ["/compile"]

FROM prod-base as debug-prod
COPY --from=debug-prod-compile /compiled-binaries /
COPY --from=pravega-dev /usr/src/gstreamer-pravega/python_apps /usr/src/gstreamer-pravega/python_apps
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH
COPY --from=pravega-dev /usr/lib/x86_64-linux-gnu/gstreamer-1.0/ /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/pravega-video-server .

FROM gstreamer-source-code as prod-compile
ENV DEBUG=false
ENV OPTIMIZATIONS=true
RUN ["/compile"]

FROM prod-base as prod
COPY --from=prod-compile /compiled-binaries /
COPY --from=pravega-dev /usr/src/gstreamer-pravega/python_apps /usr/src/gstreamer-pravega/python_apps
ENV PATH=/usr/src/gstreamer-pravega/python_apps:$PATH
COPY --from=pravega-dev /usr/lib/x86_64-linux-gnu/gstreamer-1.0/ /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
COPY --from=pravega-dev /usr/src/gstreamer-pravega/target/release/pravega-video-server .

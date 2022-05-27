#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# This Docker container can be used for interactive development environments including Visual Studio Code.
# See associated Helm chart in ../k8s/charts/deepstream-dev-pod.

ARG FROM_IMAGE=nvcr.io/nvidia/deepstream:5.1-21.02-devel

FROM ${FROM_IMAGE}

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

RUN apt-key adv --fetch-keys http://developer.download.nvidia.com/compute/cuda/repos/ubuntu1804/x86_64/3bf863cc.pub

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        curl \
        dnsutils \
        iproute2 \
        iputils-ping \
        less \
        nano \
        netcat \
        net-tools \
        openjdk-11-jdk \
        openssh-server \
        sudo \
        wget

RUN mkdir /var/run/sshd
RUN useradd -rm -d /home/ubuntu -s /bin/bash -g root -G sudo -u 1001 ubuntu
RUN echo "ubuntu ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

# Install Python Bindings for DeepStream.
RUN apt-get install -y --no-install-recommends \
        gir1.2-gst-rtsp-server-1.0 \
        gobject-introspection \
        gstreamer1.0-rtsp \
        libgirepository1.0-dev \
        libgstrtspserver-1.0-0 \
        python3-configargparse \
        python3-dev \
        python3-gi \
        python3-gst-1.0 \
        python3-numpy \
        python3-opencv \
        python3-pip
        
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

# Switch to non-root user.
USER ubuntu

ARG RUST_JOBS=4

WORKDIR /home/ubuntu

# Build gstreamer-pravega components.
# We'll start with a clone of the Github repo to allow developers to push changes.

RUN git clone --recursive https://github.com/pravega/gstreamer-pravega
WORKDIR /home/ubuntu/gstreamer-pravega
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# Copy any changes and rebuild. This should be fast because only updated files will be compiled.
COPY Cargo.toml .
COPY Cargo.lock .
COPY apps apps
COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter
COPY gst-plugin-pravega gst-plugin-pravega
COPY integration-test integration-test
COPY pravega-video pravega-video
COPY pravega-video-server pravega-video-server
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# Install compiled gstreamer-pravega libraries.
USER 0
RUN mv -v target/release/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
RUN mv -v target/release/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/
USER ubuntu

# Entrypoint will start sshd.
COPY docker/devpod-entrypoint.sh /entrypoint.sh
COPY --chown=ubuntu:root docker/sshd_config /home/ubuntu/.ssh/sshd_config
CMD ["/entrypoint.sh"]

WORKDIR /home/ubuntu

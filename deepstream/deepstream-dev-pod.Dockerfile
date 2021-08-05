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

# Default user will be ubuntu.
USER ubuntu
WORKDIR /home/ubuntu

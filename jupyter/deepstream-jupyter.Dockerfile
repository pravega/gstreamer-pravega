#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# Add JupyterHub and Pravega to DeepStream.
# Based on 
#   - https://github.com/jupyter/docker-stacks/blob/master/base-notebook/Dockerfile
#   - https://github.com/rust-lang/docker-rust-nightly/blob/master/buster/Dockerfile
#   - https://hub.docker.com/layers/rust/library/rust/1.49.0/images/sha256-71e239392f5a70bc034522a089175bd36d1344205625047ed42722a205b683b2?context=explore

ARG FROM_IMAGE=nvcr.io/nvidia/deepstream:5.1-21.02-devel

FROM ${FROM_IMAGE}

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

# Fix DL4006
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

USER root

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update --yes && \
    apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        dnsutils \
        fonts-liberation \
        iproute2 \
        iputils-ping \
        less \
        locales \
        nano \
        netcat \
        net-tools \
        python3-dev \
        python3-gi \
        python3-gst-1.0 \
        python3-pip \
        python3-setuptools \
        run-one \
        sudo \
        openjdk-11-jdk \
        wget

# Install Python Bindings for DeepStream.
RUN cd /opt/nvidia/deepstream/deepstream/lib && \
    python3 setup.py install

# Must upgrade pip for jupyterhub.
RUN pip3 install --upgrade pip

# Install dependencies for applications.
RUN python3 -m pip install \
        configargparse \
        jupyterhub \
        jupyterlab \
        notebook

# Install DeepStream sample apps.
RUN cd /opt/nvidia/deepstream/deepstream/sources && \
    git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps

RUN echo "en_US.UTF-8 UTF-8" > /etc/locale.gen && \
    locale-gen

# Configure environment for Jupyter.
ARG NB_USER="jovyan"
ARG NB_UID="1000"
ARG NB_GID="100"
ENV SHELL=/bin/bash \
    NB_USER="${NB_USER}" \
    NB_UID=${NB_UID} \
    NB_GID=${NB_GID} \
    LC_ALL=en_US.UTF-8 \
    LANG=en_US.UTF-8 \
    LANGUAGE=en_US.UTF-8 \
    REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt

# Install Rust compiler.
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

# Copy a script that we will use to correct permissions after running certain commands
COPY jupyter/fix-permissions /usr/local/bin/fix-permissions
RUN chmod a+rx /usr/local/bin/fix-permissions

# Enable prompt color in the skeleton .bashrc before creating the default NB_USER
# hadolint ignore=SC2016
RUN sed -i 's/^#force_color_prompt=yes/force_color_prompt=yes/' /etc/skel/.bashrc

# Create NB_USER with name jovyan user with UID=1000 and in the 'users' group
# and make sure these dirs are writable by the `users` group.
RUN echo "auth requisite pam_deny.so" >> /etc/pam.d/su && \
    sed -i.bak -e 's/^%admin/#%admin/' /etc/sudoers && \
    sed -i.bak -e 's/^%sudo/#%sudo/' /etc/sudoers && \
    useradd -l -m -s /bin/bash -N -u "${NB_UID}" "${NB_USER}" && \
    chmod g+w /etc/passwd && \
    fix-permissions "${HOME}"

# Allow jovyan to execute sudo for reconfiguration from Jupyter.
RUN echo "${NB_USER} ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

USER ${NB_UID}

ENV HOME="/home/${NB_USER}"

# Install Jupyter Notebook, Lab, and Hub
# Generate a notebook server config
# Cleanup temporary files
# Correct permissions
# Do all this in a single RUN command to avoid duplicating all of the
# files across image layers when the permissions change
RUN jupyter notebook --generate-config && \
    jupyter lab clean && \
    rm -rf "/home/${NB_USER}/.cache/yarn" && \
    fix-permissions "/home/${NB_USER}"

EXPOSE 8888

# Configure container startup
# ENTRYPOINT ["tini", "-g", "--"]
CMD ["start-notebook.sh"]

# Copy local files as late as possible to avoid cache busting
COPY --chown=$NB_UID:$NB_GID jupyter/start.sh jupyter/start-notebook.sh jupyter/start-singleuser.sh /usr/local/bin/
# Currently need to have both jupyter_notebook_config and jupyter_server_config to support classic and lab
COPY --chown=$NB_UID:$NB_GID jupyter/jupyter_notebook_config.py /etc/jupyter/

# Fix permissions on /etc/jupyter as root
USER root

# Prepare upgrade to JupyterLab V3.0 #1205
RUN sed -re "s/c.NotebookApp/c.ServerApp/g" \
    /etc/jupyter/jupyter_notebook_config.py > /etc/jupyter/jupyter_server_config.py && \
    fix-permissions /etc/jupyter/

USER ${NB_UID}

WORKDIR "${HOME}"

# Build gstreamer-pravega components.
# We'll start with a clone of the Github repo to allow developers to push changes.

RUN git clone --recursive https://github.com/pravega/gstreamer-pravega
WORKDIR ${HOME}/gstreamer-pravega
ARG RUST_JOBS=4
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# Copy any changes and rebuild. This should be fast because only updated files will be compiled.
COPY --chown=$NB_UID:$NB_GID Cargo.toml .
COPY --chown=$NB_UID:$NB_GID Cargo.lock .
COPY --chown=$NB_UID:$NB_GID apps apps
COPY --chown=$NB_UID:$NB_GID deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter
COPY --chown=$NB_UID:$NB_GID gst-plugin-pravega gst-plugin-pravega
COPY --chown=$NB_UID:$NB_GID integration-test integration-test
COPY --chown=$NB_UID:$NB_GID pravega-video pravega-video
COPY --chown=$NB_UID:$NB_GID pravega-video-server pravega-video-server
RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# Copy gstreamer-pravega libraries and applications.
COPY --chown=$NB_UID:$NB_GID deepstream deepstream
COPY --chown=$NB_UID:$NB_GID python_apps python_apps
ENV PYTHONPATH=${HOME}/gstreamer-pravega/python_apps/lib

# Install compiled gstreamer-pravega libraries.
USER root
RUN mv -v target/release/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
RUN mv -v target/release/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/

# Switch back to jovyan to avoid accidental container runs as root.
USER ${NB_UID}

WORKDIR "${HOME}"

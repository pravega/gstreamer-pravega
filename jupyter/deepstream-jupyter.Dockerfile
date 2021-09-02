#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# Add JupyterHub to DeepStream.
# Based on https://github.com/jupyter/docker-stacks/blob/master/base-notebook/Dockerfile

ARG FROM_IMAGE=nvcr.io/nvidia/deepstream:5.1-21.02-devel

FROM deepstream-dev-pod:faheyc AS builder

FROM ${FROM_IMAGE}

ARG NB_USER="jovyan"
ARG NB_UID="1000"
ARG NB_GID="100"
# pyds requires Python 3.6
ARG PYTHON_VERSION=3.6

COPY docker/ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

# Fix DL4006
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

USER root

ENV DEBIAN_FRONTEND noninteractive
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
        run-one \
        sudo \
        openjdk-11-jdk \
        wget

RUN echo "en_US.UTF-8 UTF-8" > /etc/locale.gen && \
    locale-gen

# Configure environment
ENV CONDA_DIR=/opt/conda \
    SHELL=/bin/bash \
    NB_USER="${NB_USER}" \
    NB_UID=${NB_UID} \
    NB_GID=${NB_GID} \
    LC_ALL=en_US.UTF-8 \
    LANG=en_US.UTF-8 \
    LANGUAGE=en_US.UTF-8 \
    REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt

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

# Copy a script that we will use to correct permissions after running certain commands
COPY jupyter/fix-permissions /usr/local/bin/fix-permissions
RUN chmod a+rx /usr/local/bin/fix-permissions

# Enable prompt color in the skeleton .bashrc before creating the default NB_USER
# hadolint ignore=SC2016
RUN sed -i 's/^#force_color_prompt=yes/force_color_prompt=yes/' /etc/skel/.bashrc && \
   # Add call to conda init script see https://stackoverflow.com/a/58081608/4413446
   echo 'eval "$(command conda shell.bash hook 2> /dev/null)"' >> /etc/skel/.bashrc

# Create NB_USER with name jovyan user with UID=1000 and in the 'users' group
# and make sure these dirs are writable by the `users` group.
RUN echo "auth requisite pam_deny.so" >> /etc/pam.d/su && \
    sed -i.bak -e 's/^%admin/#%admin/' /etc/sudoers && \
    sed -i.bak -e 's/^%sudo/#%sudo/' /etc/sudoers && \
    useradd -l -m -s /bin/bash -N -u "${NB_UID}" "${NB_USER}" && \
    mkdir -p "${CONDA_DIR}" && \
    chown "${NB_USER}:${NB_GID}" "${CONDA_DIR}" && \
    chmod g+w /etc/passwd && \
    fix-permissions "${HOME}" && \
    fix-permissions "${CONDA_DIR}"

# Allow notebook user to update DeepStream and GStreamer libraries.
RUN chown -R ${NB_USER} \
    /opt/nvidia/deepstream \
    /usr/lib/x86_64-linux-gnu/gstreamer-1.0

# Allow normal user to execute sudo.
RUN echo "${NB_USER} ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

USER ${NB_UID}

ENV PATH="${CONDA_DIR}/bin:${PATH}" \
    HOME="/home/${NB_USER}"

# Setup work directory for backward-compatibility
RUN mkdir "/home/${NB_USER}/work" && \
    fix-permissions "/home/${NB_USER}"

# Install conda as jovyan and check the sha256 sum provided on the download site
WORKDIR /tmp

# ---- Miniforge installer ----
# Check https://github.com/conda-forge/miniforge/releases
# Package Manager and Python implementation to use (https://github.com/conda-forge/miniforge)
# We're using Mambaforge installer, possible options:
# - conda only: either Miniforge3 to use Python or Miniforge-pypy3 to use PyPy
# - conda + mamba: either Mambaforge to use Python or Mambaforge-pypy3 to use PyPy
# Installation: conda, mamba, pip
RUN set -x && \
    # Miniforge installer
    miniforge_arch=$(uname -m) && \
    miniforge_installer="Mambaforge-Linux-${miniforge_arch}.sh" && \
    wget --quiet "https://github.com/conda-forge/miniforge/releases/latest/download/${miniforge_installer}" && \
    /bin/bash "${miniforge_installer}" -f -b -p "${CONDA_DIR}" && \
    rm "${miniforge_installer}" && \
    # Conda configuration see https://conda.io/projects/conda/en/latest/configuration.html
    conda config --system --set auto_update_conda false && \
    conda config --system --set show_channel_urls true && \
    if [[ "${PYTHON_VERSION}" != "default" ]]; then mamba install --quiet --yes python="${PYTHON_VERSION}"; fi && \
    mamba list python | grep '^python ' | tr -s ' ' | cut -d ' ' -f 1,2 >> "${CONDA_DIR}/conda-meta/pinned" && \
    # Using conda to update all packages: https://github.com/mamba-org/mamba/issues/1092
    conda update --all --quiet --yes && \
    conda clean --all -f -y && \
    rm -rf "/home/${NB_USER}/.cache/yarn" && \
    fix-permissions "${CONDA_DIR}" && \
    fix-permissions "/home/${NB_USER}"

# Install Jupyter Notebook, Lab, and Hub
# Generate a notebook server config
# Cleanup temporary files
# Correct permissions
# Do all this in a single RUN command to avoid duplicating all of the
# files across image layers when the permissions change
RUN mamba install --quiet --yes \
    'notebook' \
    'jupyterhub' \
    'jupyterlab' && \
    mamba clean --all -f -y && \
    npm cache clean --force && \
    jupyter notebook --generate-config && \
    jupyter lab clean && \
    rm -rf "/home/${NB_USER}/.cache/yarn" && \
    fix-permissions "${CONDA_DIR}" && \
    fix-permissions "/home/${NB_USER}"

EXPOSE 8888

# # Configure container startup
# ENTRYPOINT ["tini", "-g", "--"]
CMD ["start-notebook.sh"]

# Copy local files as late as possible to avoid cache busting
COPY jupyter/start.sh jupyter/start-notebook.sh jupyter/start-singleuser.sh /usr/local/bin/
# Currently need to have both jupyter_notebook_config and jupyter_server_config to support classic and lab
COPY jupyter/jupyter_notebook_config.py /etc/jupyter/

# Fix permissions on /etc/jupyter as root
USER root

# Prepare upgrade to JupyterLab V3.0 #1205
RUN sed -re "s/c.NotebookApp/c.ServerApp/g" \
    /etc/jupyter/jupyter_notebook_config.py > /etc/jupyter/jupyter_server_config.py && \
    fix-permissions /etc/jupyter/

USER ${NB_UID}

# Install Python Bindings for DeepStream.
RUN mamba install --yes \
        configargparse \
        gst-python \
        gobject-introspection \
        pygobject

RUN cd /opt/nvidia/deepstream/deepstream/lib && \
    python3 setup.py install && \
    cd /opt/nvidia/deepstream/deepstream/sources && \
    git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps

ARG RUST_JOBS=4

WORKDIR "${HOME}"

# Build gstreamer-pravega components.
# We'll start with a clone of the Github repo to allow developers to push changes.

# RUN git clone --recursive https://github.com/pravega/gstreamer-pravega
# WORKDIR ${HOME}/gstreamer-pravega
# RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
# RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# # Copy any changes and rebuild. This should be fast because only updated files will be compiled.
# COPY Cargo.toml .
# COPY Cargo.lock .
# COPY apps apps
# COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter
# COPY gst-plugin-pravega gst-plugin-pravega
# COPY integration-test integration-test
# COPY pravega-video pravega-video
# COPY pravega-video-server pravega-video-server
# RUN cargo build --package gst-plugin-pravega --locked --release --jobs ${RUST_JOBS}
# RUN cargo build --package pravega_protocol_adapter --locked --release --jobs ${RUST_JOBS}

# # Install compiled gstreamer-pravega libraries.
# RUN mv -v target/release/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
# RUN mv -v target/release/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/

COPY --from=builder --chown=${NB_USER}:root /usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstpravega.so
COPY --from=builder --chown=${NB_USER}:root /opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so
USER root
# RUN chown
# RUN ldconfig
USER ${NB_UID}

# Switch back to jovyan to avoid accidental container runs as root
USER ${NB_UID}

WORKDIR "${HOME}"

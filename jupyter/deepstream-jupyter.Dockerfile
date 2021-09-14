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

# JupyterHub will use Python 3.9 in Conda.
# OS Python 3.6 will be an available kernel for notebooks that require DeepStream.

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

# Upgrade pip.
RUN pip3 install --upgrade pip

# Install Python Bindings for DeepStream.
RUN cd /opt/nvidia/deepstream/deepstream/lib && \
    python3 setup.py install

# Install dependencies for applications.
RUN python3 -m pip install \
        configargparse \
        ipykernel

# Install DeepStream sample apps.
RUN cd /opt/nvidia/deepstream/deepstream/sources && \
    git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps

RUN echo "en_US.UTF-8 UTF-8" > /etc/locale.gen && \
    locale-gen

# Configure environment for Jupyter.
ARG PYTHON_VERSION=3.9
ARG NB_USER="jovyan"
ARG NB_UID="1000"
ARG NB_GID="100"
ENV CONDA_DIR=/opt/conda \
    SHELL=/bin/bash \
    NB_USER="${NB_USER}" \
    NB_UID=${NB_UID} \
    NB_GID=${NB_GID} \
    LC_ALL=en_US.UTF-8 \
    LANG=en_US.UTF-8 \
    LANGUAGE=en_US.UTF-8 \
    REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt

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
    mkdir -p "${CONDA_DIR}" && \
    chown "${NB_USER}:${NB_GID}" "${CONDA_DIR}" && \
    chmod g+w /etc/passwd && \
    fix-permissions "${HOME}" && \
    fix-permissions "${CONDA_DIR}"

# Allow jovyan to execute sudo for reconfiguration from Jupyter.
RUN echo "${NB_USER} ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

USER ${NB_UID}

ENV HOME="/home/${NB_USER}" \
    PATH="${CONDA_DIR}/bin:${PATH}"

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

# Add OS python as an available IPython kernel.
RUN python3 -m ipykernel install --user --name deepstream --display-name DeepStream

EXPOSE 8888

# Configure container startup.
CMD ["start-notebook.sh"]

# Copy local files as late as possible to avoid cache busting.
COPY --chown=$NB_UID:$NB_GID jupyter/start.sh jupyter/start-notebook.sh jupyter/start-singleuser.sh /usr/local/bin/
# Currently need to have both jupyter_notebook_config and jupyter_server_config to support classic and lab.
COPY --chown=$NB_UID:$NB_GID jupyter/jupyter_notebook_config.py /etc/jupyter/

# Fix permissions on /etc/jupyter as root
USER root

# Prepare upgrade to JupyterLab V3.0 #1205
RUN sed -re "s/c.NotebookApp/c.ServerApp/g" \
    /etc/jupyter/jupyter_notebook_config.py > /etc/jupyter/jupyter_server_config.py && \
    fix-permissions /etc/jupyter/

USER ${NB_UID}

WORKDIR "${HOME}"

########################################################################################
# Install Rust compiler.
########################################################################################

USER root

WORKDIR /tmp

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

USER ${NB_UID}

WORKDIR "${HOME}"

########################################################################################
# https://github.com/jupyter/docker-stacks/blob/master/scipy-notebook/Dockerfile
########################################################################################

USER root

# ffmpeg for matplotlib anim & dvipng+cm-super for latex labels
RUN apt-get update --yes && \
    apt-get install --yes --no-install-recommends ffmpeg dvipng cm-super && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

USER ${NB_UID}

# Install Python 3 packages
RUN mamba install --quiet --yes \
    'altair' \
    'beautifulsoup4' \
    'bokeh' \
    'bottleneck' \
    'cloudpickle' \
    'conda-forge::blas=*=openblas' \
    'cython' \
    'dask' \
    'dill' \
    'h5py' \
    'ipympl'\
    'ipywidgets' \
    'matplotlib-base' \
    'numba' \
    'numexpr' \
    'pandas' \
    'patsy' \
    'protobuf' \
    'pytables' \
    'scikit-image' \
    'scikit-learn' \
    'scipy' \
    'seaborn' \
    'sqlalchemy' \
    'statsmodels' \
    'sympy' \
    'widgetsnbextension'\
    'xlrd' && \
    mamba clean --all -f -y && \
    fix-permissions "${CONDA_DIR}" && \
    fix-permissions "/home/${NB_USER}"

# Install facets which does not have a pip or conda package at the moment
WORKDIR /tmp
RUN git clone https://github.com/PAIR-code/facets.git && \
    jupyter nbextension install facets/facets-dist/ --sys-prefix && \
    rm -rf /tmp/facets && \
    fix-permissions "${CONDA_DIR}" && \
    fix-permissions "/home/${NB_USER}"

# Import matplotlib the first time to build the font cache.
ENV XDG_CACHE_HOME="/home/${NB_USER}/.cache/"

RUN MPLBACKEND=Agg python -c "import matplotlib.pyplot" && \
    fix-permissions "/home/${NB_USER}"

USER ${NB_UID}

WORKDIR "${HOME}"

########################################################################################
# https://github.com/jupyter/docker-stacks/blob/master/pyspark-notebook/Dockerfile
########################################################################################

USER root

# Spark dependencies
# Default values can be overridden at build time
# (ARGS are in lower case to distinguish them from ENV)
ARG spark_version="3.1.2"
ARG hadoop_version="3.2"
ARG spark_checksum="2385CB772F21B014CE2ABD6B8F5E815721580D6E8BC42A26D70BBCDDA8D303D886A6F12B36D40F6971B5547B70FAE62B5A96146F0421CB93D4E51491308EF5D5"
ARG openjdk_version="11"

ENV APACHE_SPARK_VERSION="${spark_version}" \
    HADOOP_VERSION="${hadoop_version}"

RUN apt-get update --yes && \
    apt-get install --yes --no-install-recommends \
    "openjdk-${openjdk_version}-jre-headless" \
    ca-certificates-java && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Spark installation
WORKDIR /tmp
RUN wget -q "https://archive.apache.org/dist/spark/spark-${APACHE_SPARK_VERSION}/spark-${APACHE_SPARK_VERSION}-bin-hadoop${HADOOP_VERSION}.tgz" && \
    echo "${spark_checksum} *spark-${APACHE_SPARK_VERSION}-bin-hadoop${HADOOP_VERSION}.tgz" | sha512sum -c - && \
    tar xzf "spark-${APACHE_SPARK_VERSION}-bin-hadoop${HADOOP_VERSION}.tgz" -C /usr/local --owner root --group root --no-same-owner && \
    rm "spark-${APACHE_SPARK_VERSION}-bin-hadoop${HADOOP_VERSION}.tgz"

WORKDIR /usr/local

# Configure Spark
ENV SPARK_HOME=/usr/local/spark
ENV SPARK_OPTS="--driver-java-options=-Xms1024M --driver-java-options=-Xmx4096M --driver-java-options=-Dlog4j.logLevel=info" \
    PATH="${PATH}:${SPARK_HOME}/bin"

RUN ln -s "spark-${APACHE_SPARK_VERSION}-bin-hadoop${HADOOP_VERSION}" spark && \
    # Add a link in the before_notebook hook in order to source automatically PYTHONPATH
    mkdir -p /usr/local/bin/before-notebook.d && \
    ln -s "${SPARK_HOME}/sbin/spark-config.sh" /usr/local/bin/before-notebook.d/spark-config.sh

# Fix Spark installation for Java 11 and Apache Arrow library
# see: https://github.com/apache/spark/pull/27356, https://spark.apache.org/docs/latest/#downloading
RUN cp -p "${SPARK_HOME}/conf/spark-defaults.conf.template" "${SPARK_HOME}/conf/spark-defaults.conf" && \
    echo 'spark.driver.extraJavaOptions -Dio.netty.tryReflectionSetAccessible=true' >> "${SPARK_HOME}/conf/spark-defaults.conf" && \
    echo 'spark.executor.extraJavaOptions -Dio.netty.tryReflectionSetAccessible=true' >> "${SPARK_HOME}/conf/spark-defaults.conf"

USER ${NB_UID}

RUN mamba install --quiet --yes \
    'findspark' \
    'pyarrow' && \
    mamba clean --all -f -y && \
    fix-permissions "${CONDA_DIR}" && \
    fix-permissions "/home/${NB_USER}"

WORKDIR "${HOME}"

########################################################################################
# Build gstreamer-pravega components.
########################################################################################

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

USER ${NB_UID}

WORKDIR "${HOME}"

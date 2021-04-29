#!/bin/bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
GSTREAMER_CHECKOUT=${GSTREAMER_CHECKOUT:-1.18.4}
RUST_JOBS=${RUST_JOBS:-4}

# Make sure to always have fresh base image
#docker pull ubuntu:20.10
pushd ${ROOT_DIR}/docker

docker build -t pravega/gstreamer:${GSTREAMER_CHECKOUT}-dev-with-source \
    --build-arg GSTREAMER_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gstreamer.git \
    --build-arg GSTREAMER_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_PLUGINS_BASE_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-base.git \
    --build-arg GST_PLUGINS_BASE_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_PLUGINS_BAD_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-bad.git \
    --build-arg GST_PLUGINS_BAD_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_PLUGINS_GOOD_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-good.git \
    --build-arg GST_PLUGINS_GOOD_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_PLUGINS_UGLY_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-ugly.git \
    --build-arg GST_PLUGINS_UGLY_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_LIBAV_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-libav.git \
    --build-arg GST_LIBAV_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg GST_RTSP_SERVER_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-rtsp-server.git \
    --build-arg GST_RTSP_SERVER_CHECKOUT=${GSTREAMER_CHECKOUT} \
    --build-arg RUST_JOBS=${RUST_JOBS} \
    --target dev-with-source \
    -f dev.Dockerfile \
    .
popd

# Build pravega-dev image which includes the source code and binaries for all applications.
docker build -t pravega/gstreamer:pravega-dev \
    --build-arg FROM_IMAGE=pravega/gstreamer:${GSTREAMER_CHECKOUT}-dev-with-source \
    --build-arg RUST_JOBS=${RUST_JOBS} \
    -f ${ROOT_DIR}/docker/pravega-dev.Dockerfile ${ROOT_DIR}


#!/bin/bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
GSTREAMER_CHECKOUT=${GSTREAMER_CHECKOUT:-1.18.3}

pushd ${ROOT_DIR}/docker
# Make sure to always have fresh base image
#docker pull ubuntu:20.10
# Install dev dependencies
docker build -t pravega/gstreamer:dev-dependencies -f Dockerfile-dev-dependencies .
# Download source code
docker build -t pravega/gstreamer:dev-downloaded \
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
    -f Dockerfile-dev-downloaded .
# Build dev image with source code included
docker build -t pravega/gstreamer:${GSTREAMER_CHECKOUT}-dev-with-source -f Dockerfile-dev-with-source .
# Build dev image with just binaries
#docker build -t pravega/gstreamer:${GSTREAMER_CHECKOUT}-dev -f Dockerfile-dev .
# Build base production image with necessary dependencies
#docker build -t pravega/gstreamer:prod-base -f Dockerfile-prod-base .
# Build production image optimized binaries and no debug symbols (-O3 LTO)
#docker build -t pravega/gstreamer:${GSTREAMER_CHECKOUT}-prod -f Dockerfile-prod .
# Build production image optimized binaries and debug symbols
#docker build -t pravega/gstreamer:${GSTREAMER_CHECKOUT}-prod-dbg -f Dockerfile-prod-dbg .
popd

docker build -t pravega/gstreamer:pravega-dev \
    --build-arg FROM_IMAGE=pravega/gstreamer:${GSTREAMER_CHECKOUT}-dev-with-source \
    -f ${ROOT_DIR}/docker/pravega-dev.Dockerfile ${ROOT_DIR}

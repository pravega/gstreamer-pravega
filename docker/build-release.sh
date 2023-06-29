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
GSTREAMER_CHECKOUT=${GSTREAMER_CHECKOUT:-1.18.5}
RUST_JOBS=${RUST_JOBS:-4}
DOCKER_REPOSITORY=${DOCKER_REPOSITORY}
FROM_IMAGE=ubuntu:20.04

# Make sure to always have fresh base image.
if [[ "${PULL_BASE}" != "0" ]]; then
    docker pull ${DOCKER_REPOSITORY}${FROM_IMAGE}
fi



# Build pravega-dev image which includes the source code and binaries for all applications.
# This is a cache hit 100%.
if [[ "${BUILD_DEV}" != "0" ]]; then
    docker build \
        -t pravega/gstreamer:master \
        --build-arg RUST_JOBS=${RUST_JOBS} \
        --build-arg DOCKER_REPOSITORY=${DOCKER_REPOSITORY} \
        --build-arg FROM_IMAGE=${FROM_IMAGE} \
        --target pravega-dev \
        -f ${ROOT_DIR}/docker/pravega.Dockerfile \
        ${ROOT_DIR}
fi

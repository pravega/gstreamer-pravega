#!/usr/bin/env bash

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
CONTAINER_NAME=$(basename -s .sh $0)
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_PORT=${CAMERA_PORT:-8554}
DOCKER_IMAGE=${DOCKER_IMAGE:-pravega/gstreamer:pravega-dev}

docker stop ${CONTAINER_NAME} || true

docker run --rm \
--name ${CONTAINER_NAME} \
-p ${CAMERA_PORT}:${CAMERA_PORT} \
-e CAMERA_PORT \
-e CAMERA_USER \
-e CAMERA_PASSWORD \
${DOCKER_IMAGE} \
rtsp-camera-simulator

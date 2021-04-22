#!/usr/bin/env bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
CONTAINER_NAME=$(basename -s .sh $0)
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_PORT=${CAMERA_PORT:-8554}
DOCKER_IMAGE=${DOCKER_IMAGE:-pravega/gstreamer:pravega-dev}

docker run --rm \
--name ${CONTAINER_NAME} \
--network host \
-e CAMERA_PORT \
-e CAMERA_USER \
-e CAMERA_PASSWORD \
${DOCKER_IMAGE} \
rtsp-camera-simulator

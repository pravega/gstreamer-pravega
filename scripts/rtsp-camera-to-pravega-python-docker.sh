#!/usr/bin/env bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
CONTAINER_NAME=$(basename -s .sh $0)
DOCKER_IMAGE=${DOCKER_IMAGE:-pravega/gstreamer:pravega-dev}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO
export RUST_BACKTRACE=1
export ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
export CAMERA_ADDRESS=${CAMERA_ADDRESS:-127.0.0.1}
export CAMERA_PASSWORD=${CAMERA_PASSWORD:?Required environment variable not set}
export CAMERA_PATH="/cam/realmonitor?channel=1&subtype=0"
export CAMERA_PORT=${CAMERA_PORT:-8554}
export CAMERA_USER=${CAMERA_USER:-user}
export PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
export PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
export PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp1}
export pravega_client_auth_keycloak=${pravega_client_auth_keycloak:-/tmp/keycloak.json}

docker run --rm \
--name ${CONTAINER_NAME} \
-v ${pravega_client_auth_keycloak}:/tmp/keycloak.json \
-e GST_DEBUG \
-e RUST_BACKTRACE \
-e ALLOW_CREATE_SCOPE \
-e CAMERA_ADDRESS \
-e CAMERA_PASSWORD \
-e CAMERA_PATH \
-e CAMERA_PORT \
-e CAMERA_USER \
-e PRAVEGA_CONTROLLER_URI \
-e PRAVEGA_SCOPE \
-e PRAVEGA_STREAM \
-e pravega_client_auth_method \
-e pravega_client_auth_keycloak=/tmp/keycloak.json \
${DOCKER_IMAGE} \
rtsp-camera-to-pravega.py

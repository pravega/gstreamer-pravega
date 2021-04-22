#!/usr/bin/env bash
set -ex

# TODO: This causes SIGSEGV on Ubuntu.

ROOT_DIR=$(readlink -f $(dirname $0)/..)
CONTAINER_NAME=$(basename -s .sh $0)
DOCKER_IMAGE=${DOCKER_IMAGE:-pravega/gstreamer:pravega-dev}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-${STREAM:-test1}}

docker stop ${CONTAINER_NAME} || true

docker run --rm \
--name ${CONTAINER_NAME} \
--privileged \
-v /tmp/.X11-unix:/tmp/.X11-unix \
-e DISPLAY=${DISPLAY} \
-e GST_DEBUG \
-e RUST_BACKTRACE \
${DOCKER_IMAGE} \
gst-launch-1.0 \
-v \
pravegasrc \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  $* \
! decodebin \
! videoconvert \
! textoverlay "text=from ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=false

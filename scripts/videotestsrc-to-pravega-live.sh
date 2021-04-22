#!/usr/bin/env bash

# TODO: For an unknown reason, the timestamp appears to progress faster than real time.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO
export PRAVEGA_VIDEO_LOG=info
export RUST_LOG=debug
export RUST_BACKTRACE=full
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
SIZE_SEC=${SIZE_SEC:-172800}
FPS=30

gst-launch-1.0 \
-v \
videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! x264enc tune=zerolatency key-int-max=${FPS} bitrate=200 \
! mpegtsmux alignment=-1 \
<<<<<<< HEAD
<<<<<<< HEAD
! pravegasink \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  controller=${PRAVEGA_CONTROLLER} \
  keycloak-file=\"${KEYCLOAK_FILE}\" \
  stream=${SCOPE}/${STREAM} \
  sync=true
=======
! pravegasink stream=${PRAVEGA_SCOPE}/${STREAM} controller=${PRAVEGA_CONTROLLER_URI} sync=true allow-create-scope=${ALLOW_CREATE_SCOPE}
>>>>>>> Change scripts to use PRAVEGA_CONTROLLER_URI and PRAVEGA_SCOPE
=======
! pravegasink stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} controller=${PRAVEGA_CONTROLLER_URI} sync=true allow-create-scope=${ALLOW_CREATE_SCOPE}
>>>>>>> Change all scripts to use PRAVEGA_STREAM instead of STREAM

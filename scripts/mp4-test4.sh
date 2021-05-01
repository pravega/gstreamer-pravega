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
LOG_FILE="/tmp/$(basename "${0}" .sh).log"
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=qtmux:INFO,pravegasink:LOG,basesink:INFO,FIXME
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-mp4-1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
SIZE_SEC=12
FPS=30

# 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
FIRST_TS=0
FRAGMENT_DURATION_MS=15
#PLAY_OFFSET=$(( -${FIRST_TS} - 1000000000000 ))
PLAY_OFFSET=0

gst-launch-1.0 \
-v \
  videotestsrc name=src is-live=true do-timestamp=true timestamp-offset=${FIRST_TS} num-buffers=$(($SIZE_SEC*$FPS)) \
! video/x-raw,width=160,height=120,framerate=30/1 \
! videoconvert \
! clockoverlay "font-desc=Sans, 48" "time-format=%F %T" \
! timeoverlay valignment=bottom "font-desc=Sans 48px" \
! videoconvert \
! x264enc key-int-max=60 tune=zerolatency \
! mp4mux streamable=true fragment-duration=${FRAGMENT_DURATION_MS} \
! pravegasink \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  keycloak-file=\"${KEYCLOAK_SERVICE_ACCOUNT_FILE}\" \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  sync=false \
|& tee ${LOG_FILE}

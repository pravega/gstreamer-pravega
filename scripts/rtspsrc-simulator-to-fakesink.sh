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
LOG_FILE=/tmp/rtspsrc-simulator-to-pravega.log
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=rtspsrcsimulator:LOG,pravegasink:LOG,basesink:INFO,identity:LOG
export PRAVEGA_VIDEO_LOG=info
export RUST_LOG=debug
export RUST_BACKTRACE=full
SIZE_SEC=${SIZE_SEC:-172800}
FPS=30

gst-launch-1.0 \
-v \
--eos-on-shutdown \
videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! rtspsrcsimulator first-pts=3800000000000000000 \
! fakesink \
$* |& tee ${LOG_FILE}

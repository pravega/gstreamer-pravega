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
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
ls -lh ${ROOT_DIR}/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:5,mpegtsbase:4,mpegtspacketizer:4"
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-camera8}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${PRAVEGA_STREAM} controller=127.0.0.1:9090 \
! decodebin \
! videoconvert \
! warptv \
! videoconvert \
! textoverlay "text=from ${PRAVEGA_STREAM} + warp" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! navseek \
! autovideosink sync=false

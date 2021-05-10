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
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen2
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test2}

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${PRAVEGA_STREAM} \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! autovideosink \
|& tee /tmp/pravega-to-screen-h264.log

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

# Copy a file in format GStreamer Data Protocol (GDP) to a Pravega stream.
# This format preserves buffer timestamps and other metadata.
# See pravega-to-gdp-file.sh.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:LOG
export PRAVEGA_VIDEO_LOG=info
export RUST_BACKTRACE=0
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
INPUT_FILE=${INPUT_FILE:?Required environment variable not set}
LOG_FILE="/tmp/$(basename "${0}" .sh).log"

gst-launch-1.0 \
-v \
filesrc \
  location=${INPUT_FILE} \
! gdpdepay \
! identity silent=false \
! pravegasink \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  sync=false \
  timestamp-mode=tai \
|& tee ${LOG_FILE}

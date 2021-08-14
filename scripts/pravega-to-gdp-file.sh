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

# Copy a Pravega stream to a file in format GStreamer Data Protocol (GDP).
# This format preserves buffer timestamps and other metadata.
# The output of this command can be copied to a Pravega stream using gdp-file-to-pravega.sh.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasrc:LOG
export PRAVEGA_VIDEO_LOG=info
export RUST_BACKTRACE=0
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
OUTPUT_FILE=${HOME}/${PRAVEGA_STREAM}.gdp
LOG_FILE="/tmp/$(basename "${0}" .sh).log"

gst-launch-1.0 \
-v \
pravegasrc \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  keycloak-file=\"${KEYCLOAK_SERVICE_ACCOUNT_FILE}\" \
  start-mode=timestamp \
  start-utc=2021-08-13T15:00:00.000Z \
  end-mode=timestamp \
  end-utc=2022-08-13T15:00:10.000Z \
! identity silent=false \
! "video/quicktime" \
! gdppay \
! filesink location=${OUTPUT_FILE} sync=false \
|& tee ${LOG_FILE}

ls -lh ${OUTPUT_FILE}

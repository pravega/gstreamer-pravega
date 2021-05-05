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
popd
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-camera8}
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-video-player
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin pravega-video-player -- --stream examples/${PRAVEGA_STREAM} --controller ${PRAVEGA_CONTROLLER_URI} $* \
|& tee /tmp/pravega-video-player.log
popd

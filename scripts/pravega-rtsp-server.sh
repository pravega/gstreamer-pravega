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
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO,rtspmedia:LOG,INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-demo18}
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-192.168.1.123:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-video-player
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin pravega-rtsp-server -- \
--scope examples \
--controller ${PRAVEGA_CONTROLLER_URI} \
$* |& tee /tmp/pravega-rtsp-server.log
popd

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
export GST_DEBUG="INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
pushd ${ROOT_DIR}/apps
cargo run --bin pravega-rtsp-server -- "videotestsrc ! x264enc ! rtph264pay name=pay0 pt=96" $* |& tee /tmp/pravega-rtsp-server.log
popd

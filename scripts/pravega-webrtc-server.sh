#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-webrtc-server
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/pravega-webrtc-server
cargo run -- -s ws://localhost:8443 $* \
|& tee /tmp/pravega-webrtc-server.log
popd

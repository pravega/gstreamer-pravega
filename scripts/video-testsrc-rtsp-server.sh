#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
pushd ${ROOT_DIR}/apps
cargo run --bin pravega-rtsp-server -- "videotestsrc ! x264enc ! rtph264pay name=pay0 pt=96" $* |& tee /tmp/pravega-rtsp-server.log
popd

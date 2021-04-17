#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export RUST_LOG=info
export RUST_BACKTRACE=1
# PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
pushd ${ROOT_DIR}/pravega-video-server
cargo run -- $* \
|& tee /tmp/pravega-video-server.log
popd

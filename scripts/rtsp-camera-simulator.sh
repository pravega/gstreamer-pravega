#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
# export GST_DEBUG="x264enc:INFO"
# export RUST_LOG=debug
# export RUST_BACKTRACE=1
export TZ=UTC
pushd ${ROOT_DIR}/apps
cargo run --bin rtsp-camera-simulator -- $* \
|& tee /tmp/rtsp-camera-simulator.log
popd

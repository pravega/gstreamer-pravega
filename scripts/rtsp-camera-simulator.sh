#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="x264enc:LOG,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
export TZ=UTC
pushd ${ROOT_DIR}/apps
cargo run --bin rtsp-camera-simulator $* \
|& tee /mnt/data/logs/rtsp-camera-simulator.log
popd

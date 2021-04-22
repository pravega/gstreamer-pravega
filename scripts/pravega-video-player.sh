#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera8}
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-video-player
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --release --bin pravega-video-player -- --stream examples/${STREAM} --controller ${PRAVEGA_CONTROLLER_URI} $* \
|& tee /mnt/data/logs/pravega-video-player.log
popd

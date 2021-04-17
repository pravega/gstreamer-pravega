#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO,rtspmedia:LOG,INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
STREAM=${STREAM:-demo18}
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-192.168.1.123:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-video-player
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin pravega-rtsp-server -- \
--scope examples \
--controller ${PRAVEGA_CONTROLLER} \
$* |& tee /tmp/pravega-rtsp-server.log
popd

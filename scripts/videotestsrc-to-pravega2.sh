#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasink:LOG,basesink:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
STREAM=${STREAM:-test2}
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/videotestsrc-to-pravega2
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin videotestsrc-to-pravega -- --stream examples/${STREAM} \
  --controller ${PRAVEGA_CONTROLLER} $* |& tee /tmp/videotestsrc-to-pravega2.log
popd

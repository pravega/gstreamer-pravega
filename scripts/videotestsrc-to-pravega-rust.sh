#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
popd
# log level can be INFO or LOG (verbose)
export GST_DEBUG="pravegasink:LOG,basesink:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=full
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/videotestsrc-to-pravega-rust
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin videotestsrc-to-pravega |& tee /tmp/videotestsrc-to-pravega-rust.log
popd

#!/usr/bin/env bash
#
# Benchmark throughput of pravegasrc.
# Before running this script, run benchmark-pravegasink.sh to generate the source stream.
# Set the environment variable PRAVEGA_STREAM to the value output by benchmark-pravegasink.sh.
#
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
popd
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:DEBUG,basesrc:INFO"
export RUST_BACKTRACE=1

source ${ROOT_DIR}/scripts/benchmark-config.sh

T0=`date +%s%N`

time gst-launch-1.0 \
-v \
  pravegasrc stream=examples/${PRAVEGA_STREAM:?Required environment variable not set} \
  end-mode=latest \
! fakesink sync=false \
|& tee /tmp/benchmark-pravegasrc.log

T1=`date +%s%N`
DT_MILLIS=$(( ($T1 - $T0) / 1000 / 1000 ))
FILESIZE=$(stat -c%s "${TMPFILE}")
THROUGHPUT_KB_PER_SEC=$(( ${FILESIZE} / ${DT_MILLIS} ))
echo PRAVEGA_STREAM=${PRAVEGA_STREAM}
echo Throughput of pravegasrc: ${THROUGHPUT_KB_PER_SEC} KB/s

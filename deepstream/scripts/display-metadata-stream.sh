#!/usr/bin/env bash
# Use Flink to display the content of the metadata stream to the console.
# Before running, clone https://github.com/pravega/flink-tools.
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}/../../flink-tools
./gradlew -PmainClass=io.pravega.flinktools.StreamToConsoleJob flink-tools:run \
--args="--input-stream examples/metadata1 --input-startAtTail true"

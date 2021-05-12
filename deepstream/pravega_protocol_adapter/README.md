<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->
# Pravega Protocol Adapter for NVIDIA DeepStream

This provides an implementation of NVIDIA DeepStream Message Broker for a Pravega event stream.
It can be used to write metadata such as inferred bounding boxes to a Pravega event stream.
It is not intended to write video or audio data.

Build and run test_pravega_proto_async.

```bash
./docker-build.sh
./docker-run.sh

cd ~/work/gstreamer-pravega/deepstream/pravega_protocol_adapter && \
make test
```

Build and run deepstream-test4-app.

```bash
pushd ~/work/gstreamer-pravega/deepstream/pravega_protocol_adapter && \
cargo build --release && \
pushd /opt/nvidia/deepstream/deepstream/sources/apps/sample_apps/deepstream-test4 && \
make && \
GST_DEBUG=nvmsgbroker:TRACE PRAVEGA_PROTOCOL_ADAPTER_LOG=trace \
./deepstream-test4-app \
-i /opt/nvidia/deepstream/deepstream/samples/streams/sample_720p.h264 \
-p /root/work/gstreamer-pravega/deepstream/pravega_protocol_adapter/target/release/libnvds_pravega_proto.so \
--conn-str "pravega://localhost:9090?fixed_routing_key=abc123" \
--topic examples/topic4 --cfg-file /dev/null
```

See [README.md](../README.md) for more information.

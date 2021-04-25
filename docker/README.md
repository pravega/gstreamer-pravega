<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->
# Docker Containers for GStreamer

## Overview

This builds container images with GStreamer and plugins pre-installed. This can be used for GStreamer applications that do not use DeepStream.

The following components are included:

- GStreamer
- gst-plugin-pravega
- pravega-video-server
- rtsp-camera-simulator
- gst-plugins-base
- gst-plugins-good
- gst-plugins-bad
- gst-plugins-ugly
- gst-libav
- gst-rtsp-server
- libnice
- Ubuntu 20.10

This is based on [https://github.com/restreamio/docker-gstreamer](https://github.com/restreamio/docker-gstreamer/tree/6cf16dc77f5d5928abecacf5005e49a3fbccf918).

## Image Types

There are 4 kinds of images that are built.

- pravega/gstreamer:latest-dev-with-source - includes unoptimized build with debug symbols and even source code it was built with
- pravega/gstreamer:latest-dev - same as above, but without source code for development purposes
- pravega/gstreamer:latest-prod - optimized (`-O3` and `LTO`) build without debug symbols for production purposes
- pravega/gstreamer:latest-prod-dbg - optimized (`-O2` only) build with debug symbols included for production purposes with better debugging experience
- pravega/gstreamer:pravega-dev - same as latest-dev-with-source, with gstreamer-pravega source code, libraries, and applications

## Build Procedure

```bash
./build-release.sh
```

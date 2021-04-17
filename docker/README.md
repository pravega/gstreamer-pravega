# Docker Container for GStreamer

This builds container images with GStreamer and plugins pre-installed. This can be used for GStreamer applications that do not use DeepStream.

This is based on [https://github.com/restreamio/docker-gstreamer](https://github.com/restreamio/docker-gstreamer/tree/6cf16dc77f5d5928abecacf5005e49a3fbccf918).

The following components are included:

- GStreamer
- gst-plugin-pravega
- gst-plugins-base
- gst-plugins-good
- gst-plugins-bad (with `msdk`)
- gst-plugins-ugly
- gst-libav
- libnice (newer version from git)

Base OS is Ubuntu 20.10.

# Builds on Docker Hub

Builds use Restream-specific patches by default, but there are also vanilla upstream builds available.

There are 4 kinds of images pushed to Docker Hub:

* pravega/gstreamer:latest-dev-with-source - includes unoptimized build with debug symbols and even source code it was built with
* pravega/gstreamer:latest-dev - same as above, but without source code for development purposes
* pravega/gstreamer:latest-prod - optimized (`-O3` and `LTO`) build without debug symbols for production purposes
* pravega/gstreamer:latest-prod-dbg - optimized (`-O2` only) build with debug symbols included for production purposes with better debugging experience

There are also above tags prefixed with build date for stable reference.

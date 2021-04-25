#!/usr/bin/env python3

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

import argparse
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version('Gst', '1.0')
from gi.repository import GObject, Gst
from common.is_aarch_64 import is_aarch64
from common.bus_call import bus_call


def main():
    parser = argparse.ArgumentParser(description='Record video from a camera to a Pravega stream')
    parser.add_argument('--bitrate_kilobytes_per_sec', type=float, default=1000.0)
    parser.add_argument('--controller', default='192.168.1.123:9090')
    parser.add_argument('--log_level', type=int, default=logging.INFO, help='10=DEBUG,20=INFO')
    # parser.add_argument('--no_create_scope', dest='create_scope', action='store_false')
    # parser.add_argument('--no_create_stream', dest='create_stream', action='store_false')
    parser.add_argument('--scope', default='examples')
    parser.add_argument('--stream', default='jetsoncamera1')
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info('args=%s' % str(args))

    # Set GStreamer plugin path
    # gst_plugin_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "gst-plugin-pravega/target/debug")
    # logging.debug("gst_plugin_dir=%s" % gst_plugin_dir)
    # os.environ["GST_PLUGIN_PATH"] = gst_plugin_dir

    # Set GStreamer log level
    os.environ["GST_DEBUG"] = "pravegasink:INFO"

    # Standard GStreamer initialization
    GObject.threads_init()
    Gst.init(None)

    # Create gstreamer elements
    # Create Pipeline element that will form a connection of other elements
    logging.info("Creating Pipeline")
    pipeline = Gst.Pipeline()
    if not pipeline:
        raise Exception('Unable to create Pipeline')

    # Source element for reading from the Jetson Nano camera
    logging.info("Creating Source")
    source = Gst.ElementFactory.make("nvarguscamerasrc", "source")
    if not source:
        raise Exception('Unable to create nvarguscamerasrc')

    caps_source = Gst.ElementFactory.make("capsfilter", "caps_source")
    if not caps_source:
        raise Exception('Unable to create capsfilter')
    caps_source.set_property('caps', Gst.Caps.from_string("video/x-raw(memory:NVMM),width=1280,height=720,framerate=30/1,format=NV12"))

    # videoconvert to make sure a superset of raw formats are supported
    logging.info("Creating Video Converter")
    vidconvsrc = Gst.ElementFactory.make("videoconvert", "vidconvsrc")
    if not vidconvsrc:
        raise Exception('Unable to create videoconvert')

    # nvvideoconvert to convert incoming raw buffers to NVMM Mem (NvBufSurface API)
    nvvidconvsrc = Gst.ElementFactory.make("nvvidconv", "nvvidconvsrc")
    if not nvvidconvsrc:
        raise Exception('Unable to create nvvidconv')

    caps_vidconvsrc = Gst.ElementFactory.make("capsfilter", "caps_vidconvsrc")
    if not caps_vidconvsrc:
        raise Exception('Unable to create capsfilter')
    caps_vidconvsrc.set_property('caps', Gst.Caps.from_string("video/x-raw(memory:NVMM)"))

    video_encoder = Gst.ElementFactory.make("nvv4l2h264enc", "video_encoder")
    if not video_encoder:
        raise Exception('Unable to create nvv4l2h264enc')
    video_encoder.set_property("maxperf-enable", 1)
    video_encoder.set_property("preset-level", 1)
    video_encoder.set_property("control-rate", 1)
    bitrate_bits_per_sec = int(8000 * args.bitrate_kilobytes_per_sec)
    video_encoder.set_property("bitrate", bitrate_bits_per_sec)

    mpegtsmux = Gst.ElementFactory.make("mpegtsmux", "mpegtsmux")
    if not mpegtsmux:
        raise Exception('Unable to create mpegtsmux')

    pravegasink = Gst.ElementFactory.make("pravegasink", "pravegasink")
    if not pravegasink:
        raise Exception('Unable to create pravegasink')
    pravegasink.set_property('controller', args.controller)
    pravegasink.set_property('stream', '%s/%s' % (args.scope, args.stream))

    logging.info("Adding elements to Pipeline")
    pipeline.add(source)
    pipeline.add(caps_source)
    pipeline.add(vidconvsrc)
    pipeline.add(nvvidconvsrc)
    pipeline.add(video_encoder)
    pipeline.add(mpegtsmux)
    pipeline.add(pravegasink)

    # we link the elements together
    logging.info("Linking elements in the Pipeline")
    source.link(caps_source)
    caps_source.link(vidconvsrc)
    vidconvsrc.link(nvvidconvsrc)
    nvvidconvsrc.link(video_encoder)
    video_encoder.link(mpegtsmux)
    mpegtsmux.link(pravegasink)

    # create an event loop and feed gstreamer bus mesages to it
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    # start play back and listen to events
    logging.info("Starting pipeline")
    pipeline.set_state(Gst.State.PLAYING)
    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements
        pipeline.set_state(Gst.State.NULL)
        raise


if __name__ == '__main__':
    main()

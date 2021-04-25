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
gi.require_version('GLib', '2.0')
gi.require_version('GObject', '2.0')
from gi.repository import GLib, GObject, Gst


def make_element(factory_name, element_name):
    logging.info("Creating element %s of type %s" % (element_name, factory_name))
    element = Gst.ElementFactory.make(factory_name, element_name)
    if not element:
        raise Exception("Unable to create element %s of type %s" % (element_name, factory_name))
    return element


def bus_call(bus, message, loop):
    t = message.type
    if t == Gst.MessageType.EOS:
        logging.info('End-of-stream')
        loop.quit()
    elif t == Gst.MessageType.WARNING:
        err, debug = message.parse_warning()
        logging.warn('%s: %s' % (err, debug))
    elif t == Gst.MessageType.ERROR:
        err, debug = message.parse_error()
        logging.error('%s: %s' % (err, debug))
        loop.quit()
    return True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-uri', required=True)
    parser.add_argument('--log-level', type=int, default=logging.INFO, help='10=DEBUG,20=INFO')
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info('args=%s' % str(args))

    # Set GStreamer log level.
    if not 'GST_DEBUG' in os.environ:
        os.environ['GST_DEBUG'] = 'WARNING,pravegasrc:DEBUG'

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    # pipeline_description = 'videotestsrc name=src ! videoconvert ! autovideosink name=sink'
    # pipeline_description = 'rtspsrc name=src ! rtph264depay ! decodebin ! videoconvert ! autovideosink name=sink'
    # logging.info('Creating pipeline: ' +  pipeline_description)
    # pipeline = Gst.parse_launch(pipeline_description)

    # Create Pipeline element that will form a connection of other elements.
    logging.info("Creating pipeline")
    pipeline = Gst.Pipeline()
    if not pipeline:
        raise Exception("Unable to create pipeline")

    source = make_element("rtspsrc", "source")
    source.set_property("location", args.source_uri)
    rtph264depay = make_element("rtph264depay", "rtph264depay")
    # h264parser = make_element("h264parse", "h264parser")
    decodebin = make_element("decodebin", "decodebin")
    videoconvert = make_element("videoconvert", "videoconvert")
    sink = make_element("autovideosink", "sink")

    logging.info("Adding elements to pipeline")
    pipeline.add(source)
    pipeline.add(rtph264depay)
    # pipeline.add(h264parser)
    pipeline.add(decodebin)
    pipeline.add(videoconvert)
    pipeline.add(sink)

    logging.info("Linking elements in the pipeline")
    source.link(rtph264depay)
    rtph264depay.link(decodebin)
    # Link decoder.src -> streammux.sink_0
    # streammux_sinkpad = streammux.get_request_pad("sink_0")
    # if not streammux_sinkpad:
    #     raise Exception("Unable to get the sink pad of streammux")
    # decoder_srcpad = decoder.get_static_pad("src")
    # decoder_srcpad.link(streammux_sinkpad)
    decodebin.link(videoconvert)
    videoconvert.link(sink)

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GLib.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect('message', bus_call, loop)

    # Start play back and listen to events.
    logging.info('Starting pipeline')
    pipeline.set_state(Gst.State.PLAYING)
    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        pipeline.set_state(Gst.State.NULL)
        raise

    pipeline.set_state(Gst.State.NULL)
    logging.info('END')


if __name__ == '__main__':
    main()

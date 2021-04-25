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

#
# Demonstrates how to run a simple Python transformation on a video in a Pravega stream.
#
# See also https://gitlab.freedesktop.org/gstreamer/gst-python/-/tree/master/examples
#

import argparse
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version('GLib', '2.0')
gi.require_version('Gst', '1.0')
from gi.repository import GLib, Gst


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
    parser = argparse.ArgumentParser(description='Pravega to screen')
    parser.add_argument('--controller', default='127.0.0.1:9090')
    parser.add_argument('--log_level', type=int, default=logging.INFO, help='10=DEBUG,20=INFO')
    parser.add_argument('--scope', default='examples')
    parser.add_argument('--stream', default='hls3')
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info('args=%s' % str(args))

    # Set GStreamer plugin path.
    script_dir = os.path.dirname(os.path.abspath(__file__))
    pravega_plugin_dir = os.path.join(script_dir, '..', 'gst-plugin-pravega', 'target', 'debug')
    logging.info('pravega_plugin_dir=%s' % pravega_plugin_dir)
    python_plugin_dir = os.path.join(script_dir, 'plugins')
    logging.info('python_plugin_dir=%s' % python_plugin_dir)
    plugin_path = ':'.join([python_plugin_dir, pravega_plugin_dir, os.environ['GST_PLUGIN_PATH']])
    logging.debug('plugin_path=%s' % plugin_path)
    os.environ['GST_PLUGIN_PATH'] = plugin_path

    # Set GStreamer log level.
    if not 'GST_DEBUG' in os.environ:
        os.environ['GST_DEBUG'] = 'pravegasrc:DEBUG,python:LOG,identity:TRACE'
        logging.info('GST_DEBUG=%s' % os.environ['GST_DEBUG'])

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    # pipeline_description = 'videotestsrc name=src ! videoconvert ! autovideosink name=sink'
    # pipeline_description = 'videotestsrc name=src ! identity_py ! fakesink'
    # pipeline_description = 'videotestsrc name=src num-buffers=2000 ! example_python_transform_tensorflow ! identity silent=false dump=true ! autovideosink'
    pipeline_description = ('pravegasrc name=src ! tsdemux ! h264parse ! avdec_h264 ! videoconvert ! ' +
                            'example_python_transform_tensorflow ! autovideosink')
    logging.info('Creating pipeline: ' +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    pravegasrc = pipeline.get_by_name('src')
    pravegasrc.set_property('controller', args.controller)
    pravegasrc.set_property('stream', '%s/%s' % (args.scope, args.stream))

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

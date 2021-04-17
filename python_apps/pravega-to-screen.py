#!/usr/bin/env python3

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
    pravega_plugin_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'gst-plugin-pravega', 'target', 'debug')
    logging.debug('pravega_plugin_dir=%s' % pravega_plugin_dir)
    os.environ['GST_PLUGIN_PATH'] = pravega_plugin_dir + ':' + os.environ['GST_PLUGIN_PATH']

    # Set GStreamer log level.
    if not 'GST_DEBUG' in os.environ:
        os.environ['GST_DEBUG'] = 'pravegasrc:DEBUG'

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    # pipeline_description = 'videotestsrc name=source ! videoconvert ! autovideosink name=sink'
    pipeline_description = 'pravegasrc name=src ! tsdemux ! h264parse ! avdec_h264 ! videoconvert ! autovideosink'
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

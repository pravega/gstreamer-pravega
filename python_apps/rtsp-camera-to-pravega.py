#!/usr/bin/env python3
#
# Capture from RTSP camera and write video to a Pravega stream.
#

import argparse
import ctypes
import logging
import os
import sys
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


def bus_call(bus, message, loop):
    """Callback for GStreamer bus messages"""
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
    parser = argparse.ArgumentParser(
        description="Capture from RTSP camera and write video to a Pravega stream")
    parser.add_argument("--controller", default="127.0.0.1:9090")
    parser.add_argument("--log_level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--scope", default="examples")
    parser.add_argument("--source-uri", required=True)
    parser.add_argument("--stream", default="camera1")
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    # Set GStreamer log level.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = ("WARNING,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO," +
            "h264parse:INFO,pravegasink:DEBUG")

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    pipeline_description = (
        "rtspsrc name=source\n" +
        # Extract H264 elementary stream
        "   ! rtph264depay\n" +
        # Must align on Access Units for mpegtsmux
        "   ! h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        # Packetize in MPEG transport stream
        "   ! mpegtsmux\n" +
        "   ! queue name=queue0\n" +
        "   ! pravegasink name=pravegasink\n" +
        "")
    logging.info("Creating pipeline:\n" +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    source = pipeline.get_by_name("source")
    source.set_property("location", args.source_uri)
    # Outgoing timestamps are calculated directly from the RTP timestamps. This mode is good for recording.
    # This will provide the RTP timestamps as PTS (and the arrival timestamps as DTS).
    # See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
    source.set_property("buffer-mode", "none")
    # Drop oldest buffers when the queue is completely filled
    source.set_property("drop-on-latency", True)
    # Set the maximum latency of the jitterbuffer (milliseconds).
    # Packets will be kept in the buffer for at most this time.
    source.set_property("latency", 2000)
    # Required to get NTP timestamps as PTS
    source.set_property("ntp-sync", True)
    # Required to get NTP timestamps as PTS
    source.set_property("ntp-time-source", "running-time")
    queue0 = pipeline.get_by_name("queue0")
    if queue0:
        queue0.set_property("max-size-buffers", 0)
        queue0.set_property("max-size-bytes", 10485760)
        queue0.set_property("max-size-time", 0)
        queue0.set_property("silent", True)
        queue0.set_property("leaky", "downstream")
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("controller", args.controller)
        pravegasink.set_property("stream", "%s/%s" % (args.scope, args.stream))
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        # Required to use NTP timestamps in PTS
        pravegasink.set_property("timestamp-mode", "ntp")

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    # Start play back and listen to events.
    logging.info("Starting pipeline")
    pipeline.set_state(Gst.State.PLAYING)
    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        pipeline.set_state(Gst.State.NULL)
        raise

    pipeline.set_state(Gst.State.NULL)
    logging.info("END")


if __name__ == "__main__":
    main()

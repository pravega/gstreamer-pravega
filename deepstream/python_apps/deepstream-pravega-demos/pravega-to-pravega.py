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
# Read video from a Pravega stream, detect objects, write metadata and/or video with on-screen display to Pravega streams.
#

import configargparse as argparse
import ctypes
import datetime
import logging
import os
import sys
import time
import traceback
import distutils.util

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst

# See https://docs.nvidia.com/metropolis/deepstream/5.0DP/python-api/
import pyds


class PravegaTimestamp():
    """This is a Python version of PravegaTimestamp in pravega-video/src/timestamp.rs."""

    # Difference between NTP and Unix epochs.
    # Equals 70 years plus 17 leap days.
    # See [https://stackoverflow.com/a/29138806/5890553].
    UNIX_TO_NTP_SECONDS = (70 * 365 + 17) * 24 * 60 * 60

    # UTC to TAI offset.
    # Below is valid for dates between 2017-01-01 and the next leap second.
    # TODO: Beyond this range, we must use a table that incorporates the leap second schedule.
    # See [https://en.wikipedia.org/wiki/International_Atomic_Time].
    UTC_TO_TAI_SECONDS = 37

    def __init__(self, nanoseconds):
        self._nanoseconds = nanoseconds

    def from_nanoseconds(nanoseconds):
        """Create a PravegaTimestamp from the number of nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI."""
        return PravegaTimestamp(nanoseconds)

    def nanoseconds(self):
        return self._nanoseconds

    def to_unix_nanoseconds(self):
        return self.nanoseconds() - self.UTC_TO_TAI_SECONDS * 1000*1000*1000

    def to_unix_seconds(self):
        return self.to_unix_nanoseconds() * 1e-9

    def to_iso_8601(self):
        seconds = self.to_unix_seconds()
        return datetime.datetime.fromtimestamp(seconds, datetime.timezone.utc).isoformat()

    def is_valid(self):
        return self.nanoseconds() > 0

    def __repr__(self):
        return "%s (%d ns)" % (self.to_iso_8601(), self.nanoseconds())


def long_to_int(l):
    value = ctypes.c_int(l & 0xffffffff).value
    return value


def bus_call(bus, message, loop):
    """Callback for GStreamer bus messages"""
    t = message.type
    if t == Gst.MessageType.EOS:
        logging.info("End-of-stream")
        loop.quit()
    elif t == Gst.MessageType.WARNING:
        err, debug = message.parse_warning()
        logging.warning("%s: %s" % (err, debug))
    elif t == Gst.MessageType.ERROR:
        err, debug = message.parse_error()
        logging.error("%s: %s" % (err, debug))
        loop.quit()
    elif t == Gst.MessageType.ELEMENT:
        details = message.get_structure().to_string()
        logging.info("%s: %s" % (message.src.name, str(details),))
    elif t == Gst.MessageType.PROPERTY_NOTIFY:
        details = message.get_structure().to_string()
        logging.debug("%s: %s" % (message.src.name, str(details),))
    return True


def make_element(factory_name, element_name):
    """Create a GStreamer element, raising an exception on failure."""
    logging.info("Creating element %s of type %s" % (element_name, factory_name))
    element = Gst.ElementFactory.make(factory_name, element_name)
    if not element:
        raise Exception("Unable to create element %s of type %s" % (element_name, factory_name))
    return element


def format_clock_time(ns):
    """Format time in nanoseconds like 01:45:35.975000000"""
    s, ns = divmod(ns, 1000000000)
    m, s = divmod(s, 60)
    h, m = divmod(m, 60)
    return "%u:%02u:%02u.%09u" % (h, m, s, ns)


def str2bool(v):
    return bool(distutils.util.strtobool(v))


def resolve_pravega_stream(stream_name, default_scope):
    if stream_name:
        if "/" in stream_name:
            return stream_name
        else:
            if not default_scope:
                raise Exception("Stream %s given without a scope but pravega-scope has not been provided" % stream_name)
            return "%s/%s" % (default_scope, stream_name)
    else:
        return None


def main():
    parser = argparse.ArgumentParser(
        description="Read video from a Pravega stream, and directly write back to Pravega streams",
        auto_env_var_prefix="")
    parser.add_argument("--allow-create-scope", type=str2bool, default=True)
    parser.add_argument("--container-format", default="mp4", help="mpegts or mp4")
    parser.add_argument("--input-stream", required=True, metavar="SCOPE/STREAM")
    parser.add_argument("--gst-debug",
        default="WARNING,pravegasrc:INFO,h264parse:LOG,pravegasink:LOG")
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope")
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--rust-log", default="warn")
    parser.add_argument("--output-video-stream",
        help="Name of output stream for video with on-screen display.", metavar="SCOPE/STREAM")

    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    args.input_stream = resolve_pravega_stream(args.input_stream, args.pravega_scope)
    args.output_video_stream = resolve_pravega_stream(args.output_video_stream, args.pravega_scope)

    # Print configuration parameters.
    for arg in vars(args):
        logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    # Set GStreamer log level.
    os.environ["GST_DEBUG"] = args.gst_debug
    # Initialize a Rust tracing subscriber which is used by the Pravega Rust Client in pravegasrc, pravegasink, and libnvds_pravega_proto.
    # Either of these environment variables may be used, depending on the load order.
    os.environ["PRAVEGA_VIDEO_LOG"] = args.rust_log

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())
    loop = GObject.MainLoop()

    if args.container_format == "mpegts":
        container_pipeline = "tsdemux name=tsdemux"
    elif args.container_format == "mp4":
        container_pipeline = "qtdemux name=qtdemux"
    else:
        raise Exception("Unsupported container-format '%s'." % args.container_format)

    pipeline_desc = (
        "pravegasrc name=pravegasrc\n" +
        "   ! " + container_pipeline + "\n" +
        "   ! h264parse name=h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        "   ! mpegtsmux \n" +\
        "   ! pravegasink name=pravegasink\n" +
        "")

    logging.info("Creating pipeline:\n" +  pipeline_desc)
    pipeline = Gst.parse_launch(pipeline_desc)

    # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
    pipeline.add_property_deep_notify_watch(None, True)

    pravegasrc = pipeline.get_by_name("pravegasrc")
    pravegasrc.set_property("controller", args.pravega_controller_uri)
    pravegasrc.set_property("stream", args.input_stream)
    pravegasrc.set_property("allow-create-scope", args.allow_create_scope)
    pravegasrc.set_property("keycloak-file", args.keycloak_service_account_file)
    # pravegasrc.set_property("start-mode", "latest")
    # pravegasrc.set_property("end-mode", "latest")

    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("allow-create-scope", args.allow_create_scope)
        pravegasink.set_property("controller", args.pravega_controller_uri)
        if args.keycloak_service_account_file:
            pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasink.set_property("stream", args.output_video_stream)
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        pravegasink.set_property("timestamp-mode", "realtime-clock")

    
    # Feed GStreamer bus messages to event loop.
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    # Start pipelines.
    logging.info("Starting pipelines")
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

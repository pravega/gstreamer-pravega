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
gi.require_version("Gst", "1.0")
gi.require_version("GLib", "2.0")
gi.require_version("GObject", "2.0")
gi.require_version("Gio", "2.0")
from gi.repository import GLib, GObject, Gst,  Gio


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


def main():
    parser = argparse.ArgumentParser(
        description="Capture from an RTSP camera and write an MP4 file")
    parser.add_argument("--camera-protocols",
        help="Allowed lower transport protocols. Can be 'tcp', 'udp-mcast', 'udp', 'http', 'tls'. " +
             "Multiple protocols can be specified by separating them with a '+'.")
    parser.add_argument("--file", required=True,
        help="Name of the MP4 file to write")
    parser.add_argument("--log-level", type=int, default=logging.DEBUG, help="10=DEBUG,20=INFO")
    parser.add_argument("--source-uri", required=True,
        help="RTSP URL in format 'rtsp://user:password@host:554/path'. " +
             "Other supported protocols include rtsps and rtspst. " +
             "Refer to https://github.com/GStreamer/gst-plugins-base/blob/1.18.5/gst-libs/gst/rtsp/gstrtspurl.c.")
    parser.add_argument("--tls-ca-file",
        help="If using TLS, specify the path to the CA certificates in PEM format")
    parser.add_argument("--tls-validation-flags", default="validate-all",
        help="0 to disable TLS validation. Run 'gst-inspect-1.0 rtspsrc' for other options.")
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))

    # Set GStreamer log level.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = "WARNING,rtspsrc:LOG"

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create Pipeline element that will form a connection of other elements.
    pipeline_description = (
        "rtspsrc name=src\n" +
        "   ! rtph264depay\n" +
        "   ! h264parse\n" +
        "   ! identity silent=false\n" +
        "   ! mp4mux streamable=true fragment-duration=1\n" +
        "   ! filesink name=sink\n"
    )
    logging.info("Creating pipeline: " +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
    pipeline.add_property_deep_notify_watch(None, True)

    src = pipeline.get_by_name("src")
    if args.camera_protocols:
        src.set_property("protocols", args.camera_protocols)
    src.set_property("location", args.source_uri)
    if args.tls_ca_file:
        tls_ca_database = Gio.TlsFileDatabase.new(args.tls_ca_file)
        src.set_property("tls-database", tls_ca_database)
    if args.tls_validation_flags:
        tls_validation_flags = int(args.tls_validation_flags) if args.tls_validation_flags.isdigit() else args.tls_validation_flags
        src.set_property("tls-validation-flags", tls_validation_flags)
    sink = pipeline.get_by_name("sink")
    sink.set_property("location", args.file)

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GLib.MainLoop()
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

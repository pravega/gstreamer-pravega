#!/usr/bin/env -S python3 -u

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
# Capture from RTSP camera and write video to a Pravega stream.
#

import configargparse as argparse
import ctypes
import distutils.util
import logging
import os
import signal
import sys
import time
import traceback
from threading import Thread
from http.server import HTTPServer, BaseHTTPRequestHandler

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


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


def on_queue_overrun(element):
    logging.warning("Queue has overflowed and data has been lost. Try increasing buffer-size-mb.")


def str2bool(v):
    return bool(distutils.util.strtobool(v))


def buffer_probe(pad, info, data):
    gst_buffer = info.get_buffer()
    if gst_buffer:
        data.update()
        logging.debug("buffer_timestamp_probe: %20s:%-8s: " % (
            pad.get_parent_element().name,
            pad.name) + data.to_string()
        )
    return Gst.PadProbeReturn.OK


def start_http_server(hostname='0.0.0.0', port=8080):
    httpd = HTTPServer((hostname, port), HealthCheckHttpHandler)
    def serve_forever(httpd):
        with httpd:  # to make sure httpd.server_close is called
            httpd.serve_forever()

    thread = Thread(target=serve_forever, args=(httpd, ))
    # flag the http server thread as daemon thread so that it can be abruptly stopped at shutdown
    thread.setDaemon(True)
    thread.start()
    logging.info('Health check server is listening on %s:%d' % (hostname, port))
    return httpd


class IdleDetector():
    def __init__(self, tolerance):
        self.update_at = time.monotonic() - tolerance
        self.idle_time = 0
        self.update_tolerance = tolerance

    def update(self):
        self.update_at = time.monotonic()
    
    def to_string(self):
        return "last update at %u seconds of the monotonic clock" % (self.update_at)
    
    def is_healthy(self):
        self.idle_time = time.monotonic() - self.update_at
        return self.idle_time < self.update_tolerance


class HealthCheckHttpHandler(BaseHTTPRequestHandler):
    idle_detector = None
    def send_code_msg(self, code, msg):
        self.send_response(code)
        self.send_header('Content-Type',
                         'text/plain; charset=utf-8')
        self.end_headers()
        self.wfile.write(msg.encode('utf-8'))

    # Any code greater than or equal to 200 and less than 400 indicates success. Any other code indicates failure
    # https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/
    def do_GET(self):
        if self.path == "/ishealthy":
            if HealthCheckHttpHandler.idle_detector.is_healthy():
                self.send_code_msg(200, "OK")
            else:
                self.send_code_msg(500, "Pipeline has been idle for %d seconds" % (HealthCheckHttpHandler.idle_detector.idle_time))
        else:
            self.send_code_msg(404, "Not Found")


def main():
    parser = argparse.ArgumentParser(
        description="Capture from RTSP camera and write video to a Pravega stream",
        auto_env_var_prefix="")
    # Note that below arguments can be passed through the environment such as PRAVEGA_CONTROLLER_URI.
    parser.add_argument("--allow-create-scope", type=str2bool, default=True)
    parser.add_argument("--buffer-size-mb", type=float, default=100.0, help='Buffer size in MiB')
    parser.add_argument("--camera-address")
    parser.add_argument("--camera-height", type=int, default=180)
    parser.add_argument("--camera-password")
    parser.add_argument("--camera-path", default="/")
    parser.add_argument("--camera-port", type=int, default=554)
    parser.add_argument("--camera-protocols")
    parser.add_argument("--camera-rate-KB-per-sec", type=float, default=25.0, help="rate in KB/sec")
    parser.add_argument("--camera-uri")
    parser.add_argument("--camera-user")
    parser.add_argument("--camera-width", type=int, default=320)
    parser.add_argument("--container-format", default="mp4", help="mpegts or mp4")
    parser.add_argument("--debugspy", type=str2bool, default=False)
    parser.add_argument("--fakesink", type=str2bool, default=False)
    parser.add_argument("--fakesource", type=str2bool, default=False)
    parser.add_argument("--fragment-duration-ms", type=int, default=1)
    parser.add_argument("--health-check-enabled", type=str2bool, default=False)
    parser.add_argument("--health-check-idle-seconds", type=float, default=120.0)
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope", required=True)
    parser.add_argument("--pravega-stream", required=True)
    parser.add_argument("--pravega-buffer-size", type=int, default=1024, help='Pravega writer buffer size in bytes')
    parser.add_argument("--pravega-retention-policy-type", default="none")
    parser.add_argument("--pravega-retention-days", type=float, default=-1.0)
    parser.add_argument("--pravega-retention-bytes", type=int, default=-1)
    parser.add_argument("--pravega-retention-maintenance-interval-seconds", type=int, default=0)
    parser.add_argument("--timestamp-source", choices=["rtcp-sender-report", "local-clock"], default="local-clock",
        help="A value of rtcp-sender-report is the most accurate since the camera effectively timestamps each frame. " +
             "However for cameras that are unable to send RTSP Sender Reports or have unreliable clocks, " +
             "local-clock can be used, in which the time offset is calculated when the first frame is received. " +
             "This will result in timestamps being incorrect by up to a few seconds.")
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("%s: BEGIN" % parser.prog)

    if args.health_check_enabled:
        start_http_server()

    # Set default GStreamer logging.
    if not "GST_DEBUG" in os.environ:
        os.environ["GST_DEBUG"] = ("WARNING,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO," +
            "h264parse:WARN,qtmux:FIXME,fragmp4pay:INFO,timestampcvt:DEBUG,pravegasink:DEBUG")

    # Set default logging for pravega-video, which sets a Rust tracing subscriber used by the Pravega Rust Client.
    if not "PRAVEGA_VIDEO_LOG" in os.environ:
        os.environ["PRAVEGA_VIDEO_LOG"] = "info"

    # Print configuration parameters.
    for arg in vars(args):
        if 'password' not in arg:
            logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    # Build camera_uri from components.
    if args.camera_uri is None:
        if args.camera_address is None:
            raise Exception("If camera-uri is empty, then camera-address is required.")
        args.camera_uri = "rtsp://%s:%d%s" % (args.camera_address, args.camera_port, args.camera_path)
    logging.info("camera_uri=%s" % args.camera_uri)

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())

    # Create GStreamer pipeline.

    if args.fakesource:
        caps = "video/x-raw,format=YUY2,width=%d,height=%d,framerate=30/1" % (args.camera_width, args.camera_height)
        kbits_per_sec = int(args.camera_rate_KB_per_sec * 8.0)
        source_desc = (
            "videotestsrc name=src is-live=true do-timestamp=true\n" +
            "   ! " + caps + "\n" +
            "   ! videoconvert\n" +
            "   ! clockoverlay\n" +
            "   ! timeoverlay\n" +
            "   ! videoconvert\n" +
            "   ! queue\n" +
            "   ! x264enc tune=zerolatency key-int-max=30 bitrate=%d\n" %(kbits_per_sec))
    else:
        source_desc = (
            "rtspsrc name=rtspsrc\n" +
            # Extract H264 elementary stream
            "   ! rtph264depay\n")

    if args.fakesink:
        sink_desc = "   ! fakesink name=fakesink\n"
    else:
        sink_desc = "   ! pravegasink name=pravegasink\n"

    if args.debugspy:
        debugspy_desc = "   ! debugspy checksum-type=md5\n"
    else:
        debugspy_desc = ""

    if args.container_format == "mpegts":
        container_pipeline = "mpegtsmux"
    elif args.container_format == "mp4":
        container_pipeline = "mp4mux ! fragmp4pay"
    else:
        raise Exception("Unsupported container-format '%s'." % args.container_format)

    pipeline_description = (
        source_desc +
        debugspy_desc +
        # Must align on Access Units
        "   ! h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        # Convert time from NTP to TAI
        "   ! timestampcvt name=timestampcvt\n" +
        "   ! " + container_pipeline + "\n"
        # Use a large queue to avoid blocking due to temporary network or system failures
        "   ! queue name=queue_sink\n" +
        sink_desc)
    logging.info("Creating pipeline:\n" +  pipeline_description)
    pipeline = Gst.parse_launch(pipeline_description)

    # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
    pipeline.add_property_deep_notify_watch(None, True)

    source = pipeline.get_by_name("rtspsrc")
    if source:
        source.set_property("location", args.camera_uri)
        if args.camera_user:
            source.set_property("user-id", args.camera_user)
        if args.camera_password:
            source.set_property("user-pw", args.camera_password)
        if args.timestamp_source == "rtcp-sender-report":
            # Outgoing timestamps are calculated directly from the RTP timestamps. This mode is good for recording.
            # This will provide the RTP timestamps as PTS (and the arrival timestamps as DTS).
            # See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
            source.set_property("buffer-mode", "none")
            # Required to get NTP timestamps as PTS.
            source.set_property("ntp-sync", True)
            # Required to get NTP timestamps as PTS.
            source.set_property("ntp-time-source", "running-time")
        # Drop oldest buffers when the queue is completely filled
        source.set_property("drop-on-latency", True)
        # Set the maximum latency of the jitterbuffer (milliseconds).
        # Packets will be kept in the buffer for at most this time.
        source.set_property("latency", 2000)
        if args.camera_protocols:
            source.set_property("protocols", args.camera_protocols)
    clockoverlay = pipeline.get_by_name("clockoverlay0")
    if clockoverlay:
        clockoverlay.set_property("font-desc", "Sans 48px")
        clockoverlay.set_property("time-format", "%F %T")
        clockoverlay.set_property("shaded-background", True)
    timeoverlay = pipeline.get_by_name("timeoverlay0")
    if timeoverlay:
        timeoverlay.set_property("font-desc", "Sans 48px")
        timeoverlay.set_property("valignment", "bottom")
        timeoverlay.set_property("shaded-background", True)
    mp4mux = pipeline.get_by_name("mp4mux0")
    if mp4mux:
        mp4mux.set_property("streamable", True)
        mp4mux.set_property("fragment-duration", args.fragment_duration_ms)
    queue_sink = pipeline.get_by_name("queue_sink")
    if queue_sink:
        queue_sink.set_property("max-size-buffers", 0)
        queue_sink.set_property("max-size-bytes", int(args.buffer_size_mb * 1024 * 1024))
        queue_sink.set_property("max-size-time", 0)
        queue_sink.set_property("silent", False)
        queue_sink.connect("overrun", on_queue_overrun)
    timestampcvt = pipeline.get_by_name("timestampcvt")
    if timestampcvt:
        if args.timestamp_source == "rtcp-sender-report":
            timestampcvt.set_property("input-timestamp-mode", "ntp")
        else:
            timestampcvt.set_property("input-timestamp-mode", "relative")
    pravegasink = pipeline.get_by_name("pravegasink")
    if pravegasink:
        pravegasink.set_property("allow-create-scope", args.allow_create_scope)
        pravegasink.set_property("controller", args.pravega_controller_uri)
        if args.keycloak_service_account_file:
            pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasink.set_property("stream", "%s/%s" % (args.pravega_scope, args.pravega_stream))
        # Always write to Pravega immediately regardless of PTS
        pravegasink.set_property("sync", False)
        pravegasink.set_property("buffer-size", args.pravega_buffer_size)
        pravegasink.set_property("retention-type", args.pravega_retention_policy_type)
        if args.pravega_retention_days > 0.0:
            pravegasink.set_property("retention-days", args.pravega_retention_days)
        if args.pravega_retention_bytes > 0:
            pravegasink.set_property("retention-bytes", args.pravega_retention_bytes)
        if args.pravega_retention_maintenance_interval_seconds > 0:
            pravegasink.set_property("retention-maintenance-interval-seconds", args.pravega_retention_maintenance_interval_seconds)
        # Required to use NTP timestamps in PTS
        if not args.fakesource:
            pravegasink.set_property("timestamp-mode", "tai")
        if args.health_check_enabled:
            idle_detector = IdleDetector(args.health_check_idle_seconds)
            HealthCheckHttpHandler.idle_detector = idle_detector
            pravegasinkpad = pravegasink.get_static_pad("sink")
            pravegasinkpad.add_probe(Gst.PadProbeType.BUFFER, buffer_probe, idle_detector)

    # Create an event loop and feed GStreamer bus messages to it.
    loop = GObject.MainLoop()
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", bus_call, loop)

    def shutdown_handler(signum, frame):
        logging.info("Shutting down due to received signal %s" % signum)
        loop.quit()

    signal.signal(signal.SIGINT, shutdown_handler)
    signal.signal(signal.SIGTERM, shutdown_handler)

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

    logging.info("Stopping pipeline")
    pipeline.set_state(Gst.State.NULL)
    logging.info("%s: END" % parser.prog)


if __name__ == "__main__":
    main()

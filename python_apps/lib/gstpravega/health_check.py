#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

import logging
import time
from threading import Thread
from http.server import HTTPServer, BaseHTTPRequestHandler
from .util import str2bool
import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


class HealthCheckServer():
    @staticmethod
    def add_arguments(parser):
        """Add arguments to an argparse instance"""
        parser.add_argument("--health-check-enabled", type=str2bool, default=False)
        parser.add_argument("--health-check-idle-seconds", type=float, default=120.0)

    def __init__(self, health_check_enabled=False, health_check_idle_seconds=120.0, hostname='0.0.0.0', port=8080, **kwargs):
        self.health_check_enabled = health_check_enabled
        self.health_check_idle_seconds = health_check_idle_seconds
        self.hostname = hostname
        self.port = port
        self.idle_detector = IdleDetector(self.health_check_idle_seconds)
        self.start()

    def start(self):
        if self.health_check_enabled:
            HealthCheckHttpHandler.idle_detector = self.idle_detector
            httpd = HTTPServer((self.hostname, self.port), HealthCheckHttpHandler)
            thread = Thread(target=self.serve_forever, args=(httpd, ))
            # flag the http server thread as daemon thread so that it can be abruptly stopped at shutdown
            thread.setDaemon(True)
            thread.start()
            logging.info('Health check server is listening on %s:%d' % (self.hostname, self.port))

    def add_probe(self, pipeline, element_name, pad_name):
        if self.health_check_enabled:
            element = pipeline.get_by_name(element_name)
            if not element:
                raise Exception("Unable to get element %s" % element_name)
            sinkpad = element.get_static_pad(pad_name)
            if not sinkpad:
                raise Exception("Unable to get %s pad of %s" % (pad_name, element_name))
            sinkpad.add_probe(Gst.PadProbeType.BUFFER, self.buffer_probe, self.idle_detector)

    def serve_forever(self, httpd):
        with httpd:  # to make sure httpd.server_close is called
            httpd.serve_forever()

    @staticmethod
    def buffer_probe(pad, info, data):
        gst_buffer = info.get_buffer()
        if gst_buffer:
            data.update()
            logging.debug("buffer_timestamp_probe: %20s:%-8s: %s" % (
                pad.get_parent_element().name,
                pad.name,
                data.to_string()))
        return Gst.PadProbeReturn.OK


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
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.end_headers()
        self.wfile.write(msg.encode("utf-8"))

    # Any code greater than or equal to 200 and less than 400 indicates success. Any other code indicates failure
    # https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/
    def do_GET(self):
        if self.path == "/ishealthy":
            if HealthCheckHttpHandler.idle_detector is None:
                self.send_code_msg(500, "No detector is set")
            elif HealthCheckHttpHandler.idle_detector.is_healthy():
                self.send_code_msg(200, "OK")
            else:
                self.send_code_msg(500, "Pipeline has been idle for %d seconds" % (HealthCheckHttpHandler.idle_detector.idle_time))
        else:
            self.send_code_msg(404, "Not Found")

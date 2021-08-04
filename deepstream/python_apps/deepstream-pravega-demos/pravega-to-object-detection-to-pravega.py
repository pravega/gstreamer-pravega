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
import logging
import os
from os import fdopen
import sys
import tempfile
import time
import traceback

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst

from gstpravega import HealthCheckServer, add_probe, bus_call, format_clock_time, glist_iterator, long_to_int, make_element, resolve_pravega_stream, str2bool, PravegaTimestamp

# See https://docs.nvidia.com/metropolis/deepstream/5.0DP/python-api/
import pyds


PGIE_CLASS_ID_NONE = -1
PGIE_CLASS_ID_VEHICLE = 0
PGIE_CLASS_ID_BICYCLE = 1
PGIE_CLASS_ID_PERSON = 2
PGIE_CLASS_ID_ROADSIGN = 3


# See https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_metadata.html
def show_metadata_probe(pad, info, user_data):
    """Buffer probe to show metadata in a buffer"""
    gst_buffer = info.get_buffer()
    if gst_buffer:
        logging.info("show_metadata_probe: %20s:%-8s: pts=%23s, dts=%23s, duration=%23s, size=%8d" % (
            pad.get_parent_element().name,
            pad.name,
            format_clock_time(gst_buffer.pts),
            format_clock_time(gst_buffer.dts),
            format_clock_time(gst_buffer.duration),
            gst_buffer.get_size()))
        batch_meta = pyds.gst_buffer_get_nvds_batch_meta(hash(gst_buffer))
        if batch_meta:
            for frame_meta_raw in glist_iterator(batch_meta.frame_meta_list):
                frame_meta = pyds.NvDsFrameMeta.cast(frame_meta_raw)
                logging.info("show_metadata_probe: %20s:%-8s: buf_pts=%s, ntp_timestamp=%s" % (
                    pad.get_parent_element().name,
                    pad.name,
                    format_clock_time(frame_meta.buf_pts),
                    str(frame_meta.ntp_timestamp)))
                for obj_meta_raw in glist_iterator(frame_meta.obj_meta_list):
                    obj_meta = pyds.NvDsObjectMeta.cast(obj_meta_raw)
                    logging.info("show_metadata_probe: obj_meta.class_id=%d" % (obj_meta.class_id,))
                for user_meta_raw in glist_iterator(frame_meta.frame_user_meta_list):
                    user_meta = pyds.NvDsUserMeta.cast(user_meta_raw)
                    logging.info("show_metadata_probe: user_meta=%s" % (str(user_meta),))
    else:
        logging.info("show_metadata_probe: %20s:%-8s: no buffer")
    return Gst.PadProbeReturn.OK


# Callback function for deep-copying an NvDsEventMsgMeta struct
def meta_copy_func(data, user_data):
    logging.debug("meta_copy_func: BEGIN")
    # Cast data to pyds.NvDsUserMeta
    user_meta = pyds.NvDsUserMeta.cast(data)
    src_meta_data = user_meta.user_meta_data
    # Cast src_meta_data to pyds.NvDsEventMsgMeta
    srcmeta = pyds.NvDsEventMsgMeta.cast(src_meta_data)
    # Duplicate the memory contents of srcmeta to dstmeta
    # First use pyds.get_ptr() to get the C address of srcmeta, then
    # use pyds.memdup() to allocate dstmeta and copy srcmeta into it.
    # pyds.memdup returns C address of the allocated duplicate.
    dstmeta_ptr = pyds.memdup(pyds.get_ptr(srcmeta), sys.getsizeof(pyds.NvDsEventMsgMeta))
    # Cast the duplicated memory to pyds.NvDsEventMsgMeta
    dstmeta = pyds.NvDsEventMsgMeta.cast(dstmeta_ptr)

    # Duplicate contents of ts field. Note that reading srcmeta.ts
    # returns its C address. This allows to memory operations to be
    # performed on it.
    dstmeta.ts = pyds.strdup(srcmeta.ts)

    # Copy the sensorStr. This field is a string property.
    # The getter (read) returns its C address. The setter (write)
    # takes string as input, allocates a string buffer and copies
    # the input string into it.
    # pyds.get_string() takes C address of a string and returns
    # the reference to a string object and the assignment inside the binder copies content.
    dstmeta.sensorStr = pyds.get_string(srcmeta.sensorStr)

    if srcmeta.objSignature.size > 0:
        dstmeta.objSignature.signature = pyds.memdup(srcmeta.objSignature.signature, srcMeta.objSignature.size)
        dstmeta.objSignature.size = srcmeta.objSignature.size

    if srcmeta.extMsgSize > 0:
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE:
            srcobj = pyds.NvDsVehicleObject.cast(srcmeta.extMsg)
            obj = pyds.alloc_nvds_vehicle_object()
            obj.type = pyds.get_string(srcobj.type)
            obj.make = pyds.get_string(srcobj.make)
            obj.model = pyds.get_string(srcobj.model)
            obj.color = pyds.get_string(srcobj.color)
            obj.license = pyds.get_string(srcobj.license)
            obj.region = pyds.get_string(srcobj.region)
            dstmeta.extMsg = obj
            dstmeta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)
        elif srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON:
            srcobj = pyds.NvDsPersonObject.cast(srcmeta.extMsg)
            obj = pyds.alloc_nvds_person_object()
            obj.age = srcobj.age
            obj.gender = pyds.get_string(srcobj.gender)
            obj.cap = pyds.get_string(srcobj.cap)
            obj.hair = pyds.get_string(srcobj.hair)
            obj.apparel = pyds.get_string(srcobj.apparel)
            dstmeta.extMsg = obj
            dstmeta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)

    logging.debug("meta_copy_func: END")
    return dstmeta


# Callback function for freeing an NvDsEventMsgMeta instance
def meta_free_func(data, user_data):
    logging.debug("meta_free_func: BEGIN")
    user_meta = pyds.NvDsUserMeta.cast(data)
    srcmeta = pyds.NvDsEventMsgMeta.cast(user_meta.user_meta_data)

    # pyds.free_buffer takes C address of a buffer and frees the memory
    # It's a NOP if the address is NULL
    pyds.free_buffer(srcmeta.ts)
    pyds.free_buffer(srcmeta.sensorStr)

    if srcmeta.objSignature.size > 0:
        pyds.free_buffer(srcmeta.objSignature.signature)
        srcmeta.objSignature.size = 0

    if srcmeta.extMsgSize > 0:
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE:
            obj = pyds.NvDsVehicleObject.cast(srcmeta.extMsg)
            pyds.free_buffer(obj.type)
            pyds.free_buffer(obj.color)
            pyds.free_buffer(obj.make)
            pyds.free_buffer(obj.model)
            pyds.free_buffer(obj.license)
            pyds.free_buffer(obj.region)
        if srcmeta.objType == pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON:
            obj = pyds.NvDsPersonObject.cast(srcmeta.extMsg)
            pyds.free_buffer(obj.gender)
            pyds.free_buffer(obj.cap)
            pyds.free_buffer(obj.hair)
            pyds.free_buffer(obj.apparel)
        pyds.free_gbuffer(srcmeta.extMsg)
        srcmeta.extMsgSize = 0
    logging.debug("meta_free_func: END")


def generate_vehicle_meta(data):
    obj = pyds.NvDsVehicleObject.cast(data)
    obj.type = "sports car"
    obj.color = "yellow/green"
    obj.make = "Ford"
    obj.model = "Mustang"
    obj.license = "HOT302"
    obj.region = "CA"
    return obj


def generate_person_meta(data):
    obj = pyds.NvDsPersonObject.cast(data)
    obj.age = 45
    obj.cap = "none"
    obj.hair = "brown"
    obj.gender = "male"
    obj.apparel= "casual"
    return obj


def create_pyds_string(s):
    """Create a C zero-terminated string from the provided Python string.
    The caller is responsible for freeing the string."""
    sb = ctypes.create_string_buffer(s.encode("utf-8"))
    return pyds.strdup(ctypes.addressof(sb))


def generate_event_msg_meta(data, class_id, pravega_timestamp):
    logging.info("generate_event_msg_meta: BEGIN")
    meta = pyds.NvDsEventMsgMeta.cast(data)
    meta.sensorId = 0
    meta.placeId = 0
    meta.moduleId = 0
    meta.sensorStr = "sensor-0"
    # We store the TAI timestamp in videoPath because there isn't a better place that is output by nvmsgconv.
    meta.videoPath = str(pravega_timestamp.nanoseconds())
    # Also store UTC timestamp as a string like "2021-01-02T22:57:10.490000+00:00".
    meta.ts = create_pyds_string(pravega_timestamp.to_iso_8601())

    # This demonstrates how to attach custom objects.
    # Any custom object as per requirement can be generated and attached
    # like NvDsVehicleObject / NvDsPersonObject. Then that object should
    # be handled in payload generator library (nvmsgconv.cpp) accordingly.
    if class_id == PGIE_CLASS_ID_VEHICLE:
        meta.type = pyds.NvDsEventType.NVDS_EVENT_MOVING
        meta.objType = pyds.NvDsObjectType.NVDS_OBJECT_TYPE_VEHICLE
        meta.objClassId = PGIE_CLASS_ID_VEHICLE
        obj = pyds.alloc_nvds_vehicle_object()
        obj = generate_vehicle_meta(obj)
        meta.extMsg = obj
        meta.extMsgSize = sys.getsizeof(pyds.NvDsVehicleObject)
    elif class_id == PGIE_CLASS_ID_PERSON:
        meta.type = pyds.NvDsEventType.NVDS_EVENT_ENTRY
        meta.objType = pyds.NvDsObjectType.NVDS_OBJECT_TYPE_PERSON
        meta.objClassId = PGIE_CLASS_ID_PERSON
        obj = pyds.alloc_nvds_person_object()
        obj = generate_person_meta(obj)
        meta.extMsg = obj
        meta.extMsgSize = sys.getsizeof(pyds.NvDsPersonObject)
    elif class_id == PGIE_CLASS_ID_NONE:
        meta.type = pyds.NvDsEventType.NVDS_EVENT_EMPTY
        meta.objType = pyds.NvDsObjectType.NVDS_OBJECT_TYPE_UNKNOWN

    logging.debug("generate_event_msg_meta: END")
    return meta


def set_event_message_meta_probe(pad, info, u_data):
    logging.info("set_event_message_meta_probe: BEGIN")
    global app_args
    gst_buffer = info.get_buffer()
    if gst_buffer:
        batch_meta = pyds.gst_buffer_get_nvds_batch_meta(hash(gst_buffer))
        if batch_meta:
            for frame_meta_raw in glist_iterator(batch_meta.frame_meta_list):
                frame_meta = pyds.NvDsFrameMeta.cast(frame_meta_raw)
                logging.info("set_event_message_meta_probe: %20s:%-8s: pts=%23s, dts=%23s, duration=%23s, size=%8d" % (
                    pad.get_parent_element().name,
                    pad.name,
                    format_clock_time(gst_buffer.pts),
                    format_clock_time(gst_buffer.dts),
                    format_clock_time(gst_buffer.duration),
                    gst_buffer.get_size()))
                pravega_timestamp = PravegaTimestamp.from_nanoseconds(frame_meta.buf_pts)
                logging.info("set_event_message_meta_probe: %20s:%-8s: buf_pts=%s, pravega_timestamp=%s, ntp_timestamp=%s" % (
                    pad.get_parent_element().name,
                    pad.name,
                    format_clock_time(frame_meta.buf_pts),
                    pravega_timestamp,
                    str(frame_meta.ntp_timestamp)))
                if not pravega_timestamp.is_valid():
                    logging.info("set_event_message_meta_probe: Timestamp %s is invalid." % pravega_timestamp)
                else:
                    added_message = False
                    for obj_meta_raw in glist_iterator(frame_meta.obj_meta_list):
                        obj_meta = pyds.NvDsObjectMeta.cast(obj_meta_raw)
                        logging.info("set_event_message_meta_probe: obj_meta.class_id=%d" % (obj_meta.class_id,))
                        # We can only identify a single object in an NvDsEventMsgMeta.
                        # For now, we identify the first object in the frame.
                        # TODO: Create multiple NvDsEventMsgMeta instances per frame or use a custom user metadata class to identify multiple objects.
                        if not added_message:
                            # Allocating an NvDsEventMsgMeta instance and getting reference
                            # to it. The underlying memory is not manged by Python so that
                            # downstream plugins can access it. Otherwise the garbage collector
                            # will free it when this probe exits.
                            msg_meta = pyds.alloc_nvds_event_msg_meta()
                            msg_meta.bbox.top = obj_meta.rect_params.top
                            msg_meta.bbox.left = obj_meta.rect_params.left
                            msg_meta.bbox.width = obj_meta.rect_params.width
                            msg_meta.bbox.height = obj_meta.rect_params.height
                            msg_meta.frameId = frame_meta.frame_num
                            msg_meta.trackingId = long_to_int(obj_meta.object_id)
                            msg_meta.confidence = obj_meta.confidence
                            msg_meta = generate_event_msg_meta(msg_meta, obj_meta.class_id, pravega_timestamp)
                            user_event_meta = pyds.nvds_acquire_user_meta_from_pool(batch_meta)
                            if user_event_meta is None:
                                raise Exception("Error in attaching event meta to buffer")
                            user_event_meta.user_meta_data = msg_meta
                            user_event_meta.base_meta.meta_type = pyds.NvDsMetaType.NVDS_EVENT_MSG_META
                            # Setting callbacks in the event msg meta. The bindings layer
                            # will wrap these callables in C functions. Currently only one
                            # set of callbacks is supported.
                            pyds.set_user_copyfunc(user_event_meta, meta_copy_func)
                            pyds.set_user_releasefunc(user_event_meta, meta_free_func)
                            pyds.nvds_add_user_meta_to_frame(frame_meta, user_event_meta)
                            added_message = True
                    if app_args.add_message_when_no_objects_found and not added_message:
                        msg_meta = pyds.alloc_nvds_event_msg_meta()
                        msg_meta.frameId = frame_meta.frame_num
                        msg_meta = generate_event_msg_meta(msg_meta, PGIE_CLASS_ID_NONE, pravega_timestamp)
                        user_event_meta = pyds.nvds_acquire_user_meta_from_pool(batch_meta)
                        if user_event_meta is None:
                            raise Exception("Error in attaching event meta to buffer")
                        user_event_meta.user_meta_data = msg_meta
                        user_event_meta.base_meta.meta_type = pyds.NvDsMetaType.NVDS_EVENT_MSG_META
                        pyds.set_user_copyfunc(user_event_meta, meta_copy_func)
                        pyds.set_user_releasefunc(user_event_meta, meta_free_func)
                        pyds.nvds_add_user_meta_to_frame(frame_meta, user_event_meta)
                        added_message = True

    logging.info("set_event_message_meta_probe: END")
    return Gst.PadProbeReturn.OK


def create_msgapi_config(in_filename, keycloak_service_account_file):
    fd, out_filename = tempfile.mkstemp(prefix="msgapi_config_", suffix=".txt", text=True)
    with fdopen(fd, "w") as out_file:
        if in_filename:
            with open(in_filename, "r") as in_file:
                for line in in_file.readlines():
                    out_file.write(line)
        out_file.write("\n[message-broker]\n")
        out_file.write("keycloak-file = %s\n" % keycloak_service_account_file)
    logging.info("Created file " + out_filename)
    return out_filename


def main():
    parser = argparse.ArgumentParser(
        description="Read video from a Pravega stream, detect objects, write metadata and/or video with on-screen display to Pravega streams",
        auto_env_var_prefix="")
    parser.add_argument("--add-message-when-no-objects-found", type=str2bool, default=False)
    parser.add_argument("--allow-create-scope", type=str2bool, default=True)
    parser.add_argument("--container-format", default="mp4", help="mpegts or mp4")
    parser.add_argument("--input-stream", required=True, metavar="SCOPE/STREAM")
    parser.add_argument("--gst-debug",
        default="WARNING,pravegasrc:LOG,h264parse:LOG,nvv4l2decoder:LOG,nvmsgconv:INFO,pravegatc:LOG")
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--pravega-controller-uri", default="tcp://127.0.0.1:9090")
    parser.add_argument("--pravega-scope")
    parser.add_argument("--keycloak-service-account-file")
    parser.add_argument("--log-level", type=int, default=logging.INFO, help="10=DEBUG,20=INFO")
    parser.add_argument("--rust-log",
        default="nvds_pravega_proto=trace,warn")
    parser.add_argument("--msgapi-config-file")
    parser.add_argument("--msgconv-config-file",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "msgconv_config.txt"))
    parser.add_argument("--output-video-stream",
        help="Name of output stream for video with on-screen display.", metavar="SCOPE/STREAM")
    parser.add_argument("--output-metadata-stream",
        help="Name of output stream for metadata.", metavar="SCOPE/STREAM")
    parser.add_argument("--pgie_config_file",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "pgie_config.txt"))
    parser.add_argument("-p", "--proto-lib", dest="proto_lib",
        help="Absolute path of adaptor library", metavar="PATH",
        default="/opt/nvidia/deepstream/deepstream/lib/libnvds_pravega_proto.so")
    parser.add_argument("--recovery-table", metavar="SCOPE/TABLE")
    parser.add_argument("--schema-type", type=int, default=0,
        help="Type of message schema (0=Full, 1=minimal), default=0", metavar="<0|1>")
    parser.add_argument("--start-mode", default="earliest")
    parser.add_argument("--start-utc")
    parser.add_argument("--width", type=int, default=640)
    HealthCheckServer.add_arguments(parser)
    args = parser.parse_args()

    logging.basicConfig(level=args.log_level)
    logging.info("args=%s" % str(args))
    logging.debug("Debug logging enabled.")

    args.input_stream = resolve_pravega_stream(args.input_stream, args.pravega_scope)
    args.output_video_stream = resolve_pravega_stream(args.output_video_stream, args.pravega_scope)
    args.output_metadata_stream = resolve_pravega_stream(args.output_metadata_stream, args.pravega_scope)
    args.recovery_table = resolve_pravega_stream(args.recovery_table, args.pravega_scope)

    # Print configuration parameters.
    for arg in vars(args):
        logging.info("argument: %s: %s" % (arg, getattr(args, arg)))

    health_check_server = HealthCheckServer(**vars(args))

    global app_args
    app_args = args

    msgapi_config_file = create_msgapi_config(args.msgapi_config_file, args.keycloak_service_account_file)

    # Set GStreamer log level.
    os.environ["GST_DEBUG"] = args.gst_debug
    # Initialize a Rust tracing subscriber which is used by the Pravega Rust Client in pravegasrc, pravegasink, and libnvds_pravega_proto.
    # Either of these environment variables may be used, depending on the load order.
    os.environ["PRAVEGA_VIDEO_LOG"] = args.rust_log
    os.environ["PRAVEGA_PROTOCOL_ADAPTER_LOG"] = args.rust_log

    # Standard GStreamer initialization.
    Gst.init(None)
    logging.info(Gst.version_string())
    loop = GObject.MainLoop()
    pipelines = []

    # Create pipelines.
    # We create 2 independent pipelines because attempting to share buffers across tees fails with a seg fault.

    if args.container_format == "mpegts":
        container_pipeline = "tsdemux name=tsdemux"
    elif args.container_format == "mp4":
        container_pipeline = "qtdemux name=qtdemux"
    else:
        raise Exception("Unsupported container-format '%s'." % args.container_format)

    if args.recovery_table:
        pravegatc_pipeline = "   ! pravegatc name=pravegatc\n"
    else:
        pravegatc_pipeline = ""

    inference_pipeline_desc = (
        "pravegasrc name=pravegasrc\n" +
        "   ! identity name=from_pravegasrc silent=false\n" +
        "   ! " + container_pipeline + "\n" +
        "   ! h264parse name=h264parse\n" +
        "   ! video/x-h264,alignment=au\n" +
        "   ! nvv4l2decoder name=decoder\n" +
        "   ! identity name=from_decoder silent=false\n" +
        "   ! queue name=q_after_decode\n" +
        "   ! streammux.sink_0\n" +
        "nvstreammux name=streammux\n" +
        "   ! queue name=q_after_streammux\n" +
        "   ! nvinfer name=pgie\n" +
        "   ! nvstreamdemux name=streamdemux\n" +
        "streamdemux.src_0\n" +
        "   ! identity name=from_streamdemux silent=false\n" +
        "")

    metadata_pipeline_desc = (
        inference_pipeline_desc +
        "   ! identity name=before_msgconv silent=false\n" +
        "   ! nvmsgconv name=msgconv\n" +
        pravegatc_pipeline +
        "   ! identity name=before_msgbroker silent=false\n" +
        "   ! nvmsgbroker name=msgbroker\n" +
        "")

    osd_pipeline_desc = (
        inference_pipeline_desc +
        "   ! nvvideoconvert\n" +
        "   ! nvdsosd\n" +
        "   ! nvvideoconvert\n" +
        # Remove buffers with no PTS because mp4mux will stop. streamdemux sometime sends buffers with NvDsMeta and no PTS.
        # (Not needed for mpegtsmux.)
        # "   ! timestampcvt\n" +
        "   ! identity name=before_encoder silent=false\n" +
        "   ! nvv4l2h264enc control-rate=1 bitrate=1000000\n" +
        "   ! identity name=after_encoder silent=false\n" +
        "   ! h264parse\n" +
        "   ! identity name=after_h264parse silent=false\n" +
        # MP4 mux does not work reliably. Use MPEG TS instead.
        # "   ! mp4mux streamable=true fragment-duration=1\n" +
        # "   ! fragmp4pay\n" +
        "   ! mpegtsmux\n" +
        pravegatc_pipeline +
        "   ! identity name=to_pravegasink silent=false\n" +
        "   ! pravegasink name=pravegasink\n" +
        "")

    if args.output_metadata_stream:
        logging.info("Creating metadata pipeline:\n" +  metadata_pipeline_desc)
        metadata_pipeline = Gst.parse_launch(metadata_pipeline_desc)
        pipelines += [metadata_pipeline]

    if args.output_video_stream:
        logging.info("Creating OSD pipeline:\n" +  osd_pipeline_desc)
        osd_pipeline = Gst.parse_launch(osd_pipeline_desc)
        pipelines += [osd_pipeline]

    for pipeline in pipelines:
        # This will cause property changes to be logged as PROPERTY_NOTIFY messages.
        pipeline.add_property_deep_notify_watch(None, True)

        pravegasrc = pipeline.get_by_name("pravegasrc")
        pravegasrc.set_property("controller", args.pravega_controller_uri)
        pravegasrc.set_property("stream", args.input_stream)
        pravegasrc.set_property("allow-create-scope", args.allow_create_scope)
        pravegasrc.set_property("keycloak-file", args.keycloak_service_account_file)
        pravegasrc.set_property("start-mode", args.start_mode)
        if args.start_utc:
            pravegasrc.set_property("start-utc", args.start_utc)
        pravegatc = pipeline.get_by_name("pravegatc")
        if pravegatc:
            pravegatc.set_property("controller", args.pravega_controller_uri)
            pravegatc.set_property("table", args.recovery_table)
            pravegatc.set_property("keycloak-file", args.keycloak_service_account_file)
        streammux = pipeline.get_by_name("streammux")
        if streammux:
            streammux.set_property("width", args.width)
            streammux.set_property("height", args.height)
            streammux.set_property("batch-size", 1)
            streammux.set_property("batched-push-timeout", 4000000)
            streammux.set_property("live-source", 1)
            streammux.set_property("attach-sys-ts", False)
        pgie = pipeline.get_by_name("pgie")
        if pgie:
            pgie.set_property("config-file-path", args.pgie_config_file)
        msgconv = pipeline.get_by_name("msgconv")
        if msgconv:
            msgconv.set_property("config", args.msgconv_config_file)
            msgconv.set_property("payload-type", args.schema_type)
        msgbroker = pipeline.get_by_name("msgbroker")
        if msgbroker:
            msgbroker.set_property("proto-lib", args.proto_lib)
            msgbroker.set_property("conn-str", args.pravega_controller_uri)
            msgbroker.set_property("config", msgapi_config_file)
            msgbroker.set_property("topic", args.output_metadata_stream)
            msgbroker.set_property("sync", False)
        pravegasink = pipeline.get_by_name("pravegasink")
        if pravegasink:
            pravegasink.set_property("allow-create-scope", args.allow_create_scope)
            pravegasink.set_property("controller", args.pravega_controller_uri)
            if args.keycloak_service_account_file:
                pravegasink.set_property("keycloak-file", args.keycloak_service_account_file)
            pravegasink.set_property("stream", args.output_video_stream)
            # Always write to Pravega immediately regardless of PTS
            pravegasink.set_property("sync", False)
            pravegasink.set_property("timestamp-mode", "tai")
        before_msgconv = pipeline.get_by_name("before_msgconv")
        if before_msgconv:
            add_probe(pipeline, "before_msgconv", set_event_message_meta_probe, pad_name='sink')
        health_check_server.add_probe(pipeline, "pravegasrc", "src")

    #
    # Start pipelines.
    #

    for pipeline in pipelines:
        # Feed GStreamer bus messages to event loop.
        bus = pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", bus_call, loop)

    logging.info("Starting pipelines")
    for p in pipelines: p.set_state(Gst.State.PLAYING)

    try:
        loop.run()
    except:
        logging.error(traceback.format_exc())
        # Cleanup GStreamer elements.
        for p in pipelines: p.set_state(Gst.State.NULL)
        raise

    for p in pipelines: p.set_state(Gst.State.NULL)
    logging.info("END")


if __name__ == "__main__":
    main()

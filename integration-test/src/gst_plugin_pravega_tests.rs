//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use gst::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::{error, info};
use crate::TestConfig;

pub fn test_playback_truncated_stream(test_config: TestConfig) {
    let controller_uri = test_config.client_config.controller_uri.0;
    let scope = test_config.scope;
    let stream_name = format!("stream1-{}", test_config.test_id);

    // Initialize GStreamer
    std::env::set_var("GST_DEBUG", "pravegasrc:LOG,pravegasink:LOG,basesink:INFO");
    gst::init().unwrap();
    gstpravega::plugin_register_static().unwrap();

    //
    // Write video stream to Pravega.
    //

    let pipeline_description = format!(
        "videotestsrc name=src is-live=true do-timestamp=true num-buffers=2 \
        ! video/x-raw,width=320,height=180,framerate=30/1 \
        ! videoconvert \
        ! x264enc key-int-max=30 bitrate=100 \
        ! mpegtsmux \
        ! pravegasink controller={controller_uri} stream={scope}/{stream_name} seal=true",
        controller_uri = controller_uri,
        scope = scope,
        stream_name = stream_name,
    );
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    // Start pipeline
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until end-of-stream or error.
    let mut eos = false;
    let bus = pipeline.get_bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => {
                eos = true;
                break;
            }
            gst::MessageView::Error(err) => {
                error!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
                );
                break;
            },
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    assert!(eos);

    //
    // Read video stream, get PTS, and validate.
    //

    let pipeline_description = format!(
        "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
        ! decodebin \
        ! appsink name=sink",
        controller_uri = controller_uri,
        scope = scope,
        stream_name = stream_name,
    );
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let sink = pipeline
        .get_by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    sink.set_property("sync", &false).unwrap();

    let read_pts = Arc::new(Mutex::new(Vec::new()));
    let read_pts_clone = read_pts.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().unwrap();
                info!("sample={:?}", sample);
                let pts = sample.get_buffer().unwrap().get_pts();
                info!("pts={}", pts);
                let mut read_timestamps = read_pts_clone.lock().unwrap();
                read_timestamps.push(pts);
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    // Start pipeline
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until end-of-stream or error.
    let mut eos = false;
    let bus = pipeline.get_bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => {
                eos = true;
                break;
            }
            gst::MessageView::Error(err) => {
                error!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
                );
                break;
            },
            _ => (),
        }
    }
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    assert!(eos);

    let read_pts = read_pts.lock().unwrap();
    info!("read_pts={:?}", read_pts);

    // Truncate video stream.

    // Read video stream, get PTS, and validate.

    // Out-of-band: Play using HLS player.

    info!("END");
}

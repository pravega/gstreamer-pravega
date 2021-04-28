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
use tracing::{error, info};
use crate::TestConfig;

pub fn test_playback_truncated_stream(test_config: TestConfig) {
    // Write video stream to Pravega.

    std::env::set_var("GST_DEBUG", "pravegasrc:LOG,pravegasink:LOG,basesink:INFO");

    // Initialize GStreamer
    gst::init().unwrap();

    gstpravega::plugin_register_static().unwrap();

    // This creates a pipeline by parsing the gst-launch pipeline syntax.
    let controller_uri = test_config.client_config.controller_uri.0;
    let scope = test_config.scope;
    let stream_name = format!("stream1-{}", test_config.test_id);
    let pipeline_description = format!(
        "videotestsrc name=src is-live=true do-timestamp=true num-buffers=1 \
        ! video/x-raw,width=320,height=180,framerate=30/1 \
        ! videoconvert \
        ! x264enc key-int-max=30 bitrate=100 \
        ! mpegtsmux \
        ! pravegasink controller={controller_uri} stream={scope}/{stream_name} allow-create-scope=true",
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
    let bus = pipeline.get_bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                error!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
                );
                break;
            }
            _ => (),
        }
    }

    // Shutdown pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");

    // Read video stream, get PTS, and validate.

    // Truncate video stream.

    // Read video stream, get PTS, and validate.

    // Out-of-band: Play using HLS player.
}

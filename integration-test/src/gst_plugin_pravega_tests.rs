//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// use anyhow::{anyhow, Error};
use gst::ClockTime;
use gst::prelude::*;
use pravega_video::timestamp::PravegaTimestamp;
// use std::sync::{Arc, Mutex};
use std::convert::TryFrom;
use tracing::{error, info, debug};
use crate::TestConfig;
use crate::utils::*;

pub fn test_playback_truncated_stream(test_config: TestConfig) {
    let controller_uri = test_config.client_config.clone().controller_uri.0;
    let scope = test_config.scope.clone();
    let stream_name = format!("stream1-{}", test_config.test_id);

    // Initialize GStreamer
    std::env::set_var("GST_DEBUG", "pravegasrc:LOG,pravegasink:LOG,basesink:INFO");
    gst::init().unwrap();
    gstpravega::plugin_register_static().unwrap();

    // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
    let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
    let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
    info!("first_timestamp={}", first_timestamp);
    let first_pts = ClockTime(first_timestamp.nanoseconds());
    info!("first_pts={}", first_pts);
    let fps = 30;
    let num_buffers = 3 * fps;

    // Write video stream to Pravega.
    let pipeline_description = format!(
        "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
        ! video/x-raw,width=320,height=180,framerate={fps}/1 \
        ! videoconvert \
        ! x264enc key-int-max=30 bitrate=100 \
        ! mpegtsmux \
        ! pravegasink controller={controller_uri} stream={scope}/{stream_name} seal=true timestamp-mode=tai sync=false",
        controller_uri = controller_uri,
        scope = scope.clone(),
        stream_name = stream_name.clone(),
        timestamp_offset = first_pts.nanoseconds().unwrap(),
        num_buffers = num_buffers,
        fps = fps,
    );
    launch_pipeline(pipeline_description).unwrap();

    // Read video stream from beginning.
    let pipeline_description = format!(
        "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
        ! decodebin \
        ! appsink name=sink sync=false",
        controller_uri = controller_uri,
        scope = scope.clone(),
        stream_name = stream_name.clone(),
    );
    let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
    info!("read_pts={:?}", read_pts);
    let num_buffers_actual = read_pts.len() as u64;
    let first_pts_actual = read_pts[0];
    let last_pts_actual = *read_pts.last().unwrap();
    let delta_pts_expected = (num_buffers - 1) * gst::SECOND / fps;
    let last_pts_expected = first_pts + delta_pts_expected;
    info!("delta_pts_expected={}", delta_pts_expected);
    info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers, first_pts, last_pts_expected);
    info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
    // TODO: Why is PTS is off by 125 ms?
    assert_between("first_pts_actual", first_pts_actual, first_pts, first_pts + 126 * gst::MSECOND);
    assert_between("last_pts_actual", last_pts_actual, last_pts_expected, last_pts_expected + 126 * gst::MSECOND);
    assert_eq!(num_buffers_actual, num_buffers);

    // Read video from 1st indexed position.
    // let src_start_pts = first_pts + 1 * gst::SECOND;
    let truncate_before_timestamp = PravegaTimestamp::from_nanoseconds((first_pts + 1 * gst::SECOND).nanoseconds());
    truncate_stream(test_config.client_config, scope.clone(), stream_name.clone(), truncate_before_timestamp);
    let pipeline_description = format!(
        "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
        ! appsink name=sink sync=false",
        controller_uri = controller_uri,
        scope = scope.clone(),
        stream_name = stream_name.clone(),
        // start_timestamp = src_start_pts.nanoseconds().unwrap(),
    );
    let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
    info!("read_pts={:?}", read_pts);
    // let num_buffers_actual = read_pts.len() as u64;
    // let first_pts_actual = read_pts[0];
    // let last_pts_actual = *read_pts.last().unwrap();
    // let first_pts_expected = src_start_pts;
    // // let delta_pts_expected = (num_buffers - 1) * gst::SECOND / fps;
    // // let last_pts_expected = first_pts + delta_pts_expected;
    // info!("delta_pts_expected={}", delta_pts_expected);
    // info!("Expected: num_buffers={}, first_pts={}, last_pts={}", "??", first_pts_expected, last_pts_expected);
    // info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
    // // TODO: Why is PTS is off by 125 ms?
    // assert_between("first_pts_actual", first_pts_actual, first_pts_expected, first_pts_expected + 126 * gst::MSECOND);
    // assert_between("last_pts_actual", last_pts_actual, last_pts_expected, last_pts_expected + 126 * gst::MSECOND);
    // // assert_eq!(num_buffers_actual, num_buffers);

    // Read video stream, get PTS, and validate.

    // TODO: Test pravegasrc start-mode=timestamp start-timestamp={start_timestamp}

    // Out-of-band: Play using HLS player.

    info!("END");
}

//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#[cfg(test)]
mod test {
    use anyhow::{anyhow, Error};
    use gst::prelude::*;
    use pravega_video::timestamp::PravegaTimestamp;
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::RTSPCameraSimulator;
    use crate::utils::*;

    #[test]
    fn test_rtsp() {
        gst_init();
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-rtsp-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let rtsp_url = std::env::var("RTSP_URL").unwrap();
        let rtsp_server = RTSPCameraSimulator::new(640, 480, 20, 10.0).unwrap();
        rtsp_server.start().unwrap();
        let rtsp_url = rtsp_server.get_url();
        info!("rtsp_url={}", rtsp_url);

        let fps = 20;
        let num_sec_to_record = 10;
        // The identity element will stop the pipeline after this many video frames.
        let num_frames_to_record = num_sec_to_record * fps;
        let num_sec_expected_min = 3;
        let num_frames_expected_min = num_sec_expected_min * fps;
        let num_frames_expected_max = num_frames_to_record;
        // We expect the RTSP camera's clock to be within 24 hours of this computer's clock.
        let expected_timestamp = PravegaTimestamp::now();
        let expected_timestamp_margin = 24*60*60 * gst::SECOND;

        info!("#### Record RTSP camera to Pravega");
        // TODO: Test with queue?: queue max-size-buffers=0 max-size-bytes=10485760 max-size-time=0 silent=true leaky=downstream
        let pipeline_description = format!("\
            rtspsrc \
              buffer-mode=none \
              drop-messages-interval=0 \
              drop-on-latency=true \
              latency=2000 \
              location={rtsp_url} \
              ntp-sync=true \
              ntp-time-source=running-time \
              rtcp-sync-send-time=false \
            ! rtph264depay \
            ! h264parse \
            ! video/x-h264,alignment=au \
            ! identity silent=false eos-after={num_frames} \
            ! mpegtsmux \
            ! pravegasink {pravega_plugin_properties} \
              seal=true sync=false \
              timestamp-mode=ntp \
            ",
           pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
           rtsp_url = rtsp_url,
           num_frames = num_frames_to_record,
        );
        let _ = launch_pipeline_and_get_summary(pipeline_description).unwrap();

        info!("#### Read recorded stream from Pravega without decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_read = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary_read={}", summary_read);
        let first_pts_read = summary_read.first_valid_pts();
        let last_pts_read = summary_read.last_valid_pts();
        info!("Expected: first_valid_pts={:?}, last_valid_pts={:?}", expected_timestamp, expected_timestamp);
        info!("Actual:   first_valid_pts={:?}, last_valid_pts={:?}", first_pts_read, last_pts_read);
        assert!(first_pts_read.is_some(), "Pipeline is not recording timestamps");
        assert_timestamp_approx_eq("first_pts_written", first_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert_timestamp_approx_eq("last_pts_written", last_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert!(summary_read.pts_range() >= num_sec_expected_min * gst::SECOND);
        assert!(summary_read.pts_range() <= 2 * num_sec_to_record * gst::SECOND);

        if false {
            info!("#### Play video stream from beginning on screen");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
                timestamp_offset = -1 * (first_pts_read.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(pipeline_description).unwrap();
        }

        info!("#### Read recorded stream from Pravega with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! decodebin \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_decoded = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary_read   ={}", summary_read);
        debug!("summary_decoded={}", summary_decoded);
        debug!("summary_decoded={:?}", summary_decoded);
        let first_pts_decoded = summary_decoded.first_valid_pts();
        let last_pts_decoded = summary_decoded.last_valid_pts();
        info!("Expected: first_valid_pts={:?}, last_valid_pts={:?}", first_pts_read, last_pts_read);
        info!("Actual:   first_valid_pts={:?}, last_valid_pts={:?}", first_pts_decoded, last_pts_decoded);
        assert!(first_pts_decoded.is_some(), "Pipeline is not recording timestamps");
        let decode_margin = 10 * gst::SECOND;
        assert_timestamp_approx_eq("first_pts_decoded", first_pts_decoded, first_pts_read, decode_margin, decode_margin);
        assert_timestamp_approx_eq("last_pts_decoded", last_pts_decoded, last_pts_read, decode_margin, decode_margin);
        assert!(summary_decoded.pts_range() >= num_sec_expected_min * gst::SECOND);
        assert!(summary_decoded.pts_range() <= 2 * num_sec_to_record * gst::SECOND);
        assert_between_u64("num_buffers", summary_decoded.num_buffers(), num_frames_expected_min, num_frames_expected_max);
        assert_between_u64("num_buffers_with_valid_pts", summary_decoded.num_buffers_with_valid_pts(), num_frames_expected_min, num_frames_expected_max);
    }
}

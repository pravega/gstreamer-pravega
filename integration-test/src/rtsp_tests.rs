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
    use pravega_video::timestamp::PravegaTimestamp;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::{start_or_get_rtsp_test_source, RTSPCameraSimulatorConfigBuilder};
    use crate::utils::*;

    #[test]
    fn test_rtsp() {
        gst_init();
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-rtsp-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let fps = 20;
        let rtsp_server_config = RTSPCameraSimulatorConfigBuilder::default()
            .fps(fps)
            .build().unwrap();
        let (rtsp_url, _rtsp_server) = start_or_get_rtsp_test_source(rtsp_server_config);
        let num_sec_to_record = 10;
        // The identity element will stop the pipeline after this many video frames.
        let num_frames_to_record = num_sec_to_record * fps;
        let num_sec_expected_min = 3;
        let num_frames_expected_min = num_sec_expected_min * fps;
        let num_frames_expected_max = num_frames_to_record;
        // We expect the RTSP camera's clock to be within 24 hours of this computer's clock.
        let expected_timestamp = PravegaTimestamp::now();
        let expected_timestamp_margin = 24*60*60 * gst::SECOND;

        info!("#### Record RTSP camera to Pravega, part 1");
        // TODO: Test with queue?: queue max-size-buffers=0 max-size-bytes=10485760 max-size-time=0 silent=true leaky=downstream
        let pipeline_description_record = format!("\
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
              sync=false \
              timestamp-mode=ntp \
            ",
           pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
           rtsp_url = rtsp_url,
           num_frames = num_frames_to_record,
        );
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

        info!("#### Read recorded stream from Pravega without decoding, part 1");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
              end-mode=latest \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_read = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_read={}", summary_read);
        let first_pts_read = summary_read.first_valid_pts();
        let last_pts_read = summary_read.last_valid_pts();
        assert!(first_pts_read.is_some(), "Pipeline is not recording timestamps");
        assert_timestamp_approx_eq("first_pts_written", first_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert_timestamp_approx_eq("last_pts_written", last_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert!(summary_read.pts_range() >= num_sec_expected_min * gst::SECOND);
        assert!(summary_read.pts_range() <= (2 * num_sec_to_record + 60) * gst::SECOND);

        info!("#### Read recorded stream from Pravega with decoding, part 1");
        let pipeline_description_decode = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
              end-mode=latest \
              ! decodebin \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_decoded = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
        debug!("summary_read   ={}", summary_read);
        debug!("summary_decoded={}", summary_decoded);
        debug!("summary_decoded={:?}", summary_decoded);
        let first_pts_decoded = summary_decoded.first_valid_pts();
        let last_pts_decoded = summary_decoded.last_valid_pts();
        assert!(first_pts_decoded.is_some(), "Pipeline is not recording timestamps");
        let decode_margin = 10 * gst::SECOND;
        assert_timestamp_approx_eq("first_pts_decoded", first_pts_decoded, first_pts_read, decode_margin, decode_margin);
        assert_timestamp_approx_eq("last_pts_decoded", last_pts_decoded, last_pts_read, decode_margin, decode_margin);
        assert!(summary_decoded.pts_range() >= num_sec_expected_min * gst::SECOND);
        assert!(summary_decoded.pts_range() <= (2 * num_sec_to_record + 60) * gst::SECOND);
        assert_between_u64("num_buffers", summary_decoded.num_buffers(), num_frames_expected_min, num_frames_expected_max);
        assert_between_u64("num_buffers_with_valid_pts", summary_decoded.num_buffers_with_valid_pts(), num_frames_expected_min, num_frames_expected_max);

        // Simulate restart of recorder.
        info!("#### Record RTSP camera to Pravega, part 2");
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

        info!("#### Read recorded stream from Pravega without decoding, part 2");
        let summary_read2 = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_read ={}", summary_read);
        debug!("summary_read2={}", summary_read2);
        // summary_read2.dump("summary_read2: ");
        let first_pts_read2 = summary_read2.first_valid_pts();
        let last_pts_read2 = summary_read2.last_valid_pts();
        let max_gap_sec = 300;
        assert_timestamp_approx_eq("first_pts_read2", first_pts_read2, first_pts_read, 0 * gst::SECOND, 0 * gst::SECOND);
        assert_timestamp_approx_eq("last_pts_read2", last_pts_read2, last_pts_read, 0 * gst::SECOND,
            (2 * num_sec_to_record + max_gap_sec) * gst::SECOND);
        assert!(summary_read2.pts_range() >= 2 * num_sec_expected_min * gst::SECOND);
        assert!(summary_read2.pts_range() <= (4 * num_sec_to_record + max_gap_sec) * gst::SECOND);

        info!("#### Read recorded stream from Pravega with decoding, part 2");
        let summary_decoded2 = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
        debug!("summary_decoded ={}", summary_decoded);
        debug!("summary_decoded2={}", summary_decoded2);
        // summary_decoded2.dump("summary_decoded2: ");
        let first_pts_decoded2 = summary_decoded2.first_valid_pts();
        let last_pts_decoded2 = summary_decoded2.last_valid_pts();
        assert_timestamp_approx_eq("first_pts_decoded2", first_pts_decoded2, first_pts_read2, decode_margin, decode_margin);
        assert_timestamp_approx_eq("last_pts_decoded2", last_pts_decoded2, last_pts_read2, decode_margin, decode_margin);
        assert!(summary_decoded2.pts_range() >= 2 * num_sec_expected_min * gst::SECOND);
        assert!(summary_decoded2.pts_range() <= (4 * num_sec_to_record + max_gap_sec) * gst::SECOND);
        assert_between_u64("num_buffers", summary_decoded2.num_buffers(),
            2 * num_frames_expected_min, 2 * num_frames_expected_max);
        assert_between_u64("num_buffers_with_valid_pts", summary_decoded2.num_buffers_with_valid_pts(),
            2 * num_frames_expected_min, 2 * num_frames_expected_max);

        let interactive = false;
        if interactive {
            info!("#### Play video stream from beginning on screen");
            info!("You should see the first part of the video play in a window, followed by a 10-30 second pause, then the next part will play.");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                  end-mode=latest \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
                timestamp_offset = -1 * (first_pts_read.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(&pipeline_description).unwrap();
        }
    }
}

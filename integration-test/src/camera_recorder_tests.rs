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
    use gst::prelude::*;
    use gst::ClockType::Realtime;
    use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, SECOND};
    use rstest::rstest;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::{start_or_get_rtsp_test_source, RTSPCameraSimulatorConfigBuilder};
    use crate::utils::*;

    /// Test ungraceful stop of camera recorder pipeline by simulate a pravegasink failure.
    /// The pipeline can continue to record the video stream after restart and the video stream can be decoded and playback.
    #[rstest]
    #[case(Realtime, 30, 15)]
    fn test_ungraceful_stop(#[case] clock_type: gst::ClockType, #[case] num_sec_to_record: u64, #[case] num_sec_to_failure: u64) {
        gst_init();
        let clock = gst::SystemClock::obtain();
        clock.set_property("clock-type", &clock_type).unwrap();
        gst::SystemClock::set_default(Some(&clock));
        let rtsp_server_config = RTSPCameraSimulatorConfigBuilder::default().fps(20).build().unwrap();
        let fps = rtsp_server_config.fps;
        let num_frames = num_sec_to_record * fps;
        let (rtsp_url, _rtsp_server) = start_or_get_rtsp_test_source(rtsp_server_config);
        let test_config = &get_test_config();
        info!("### BEGIN:test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let pipeline_description = format!("\
            rtspsrc \
              buffer-mode=none \
              drop-on-latency=true \
              latency=2000 \
              location={rtsp_url} \
              ntp-sync=true \
              ntp-time-source=running-time \
            ! rtph264depay \
            ! h264parse \
            ! video/x-h264,alignment=au \
            ! identity name=h264par silent=false eos-after={num_frames} \
            ! timestampcvt \
            ! identity name=tscvt__ silent=false \
            ! mp4mux streamable=true fragment-duration=1 ! fragmp4pay \
            ! pravegasink {pravega_plugin_properties} \
              timestamp-mode=tai sync=false \
            ",
            rtsp_url = rtsp_url,
            num_frames = num_frames,
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let expected_timestamp = PravegaTimestamp::now();
        let _ = launch_pipeline_and_get_summary(format!("{} simulate-failure-after-sec={}", pipeline_description, num_sec_to_failure).as_ref());

        // restart the pipeline
        let _ = launch_pipeline_and_get_summary(&pipeline_description).unwrap();

        info!("#### Read recorded stream from Pravega, no demux, no decoding, part 1");
        let pipeline_description_read = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
              end-mode=latest \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_read = launch_pipeline_and_get_summary(&pipeline_description_read).unwrap();
        debug!("summary_read={}", summary_read);
        let first_pts_read = summary_read.first_valid_pts();
        let last_pts_read = summary_read.last_valid_pts();
        assert!(first_pts_read.is_some(), "Pipeline is not recording timestamps");
        assert_between_u64("decreasing_pts_count", summary_read.decreasing_pts_count(), 0, 0);
        assert_between_u64("decreasing_dts_count", summary_read.decreasing_dts_count(), 0, 0);
        // the gap between fisrt_pts_read and expected_timestamp is caused by the duration of creation of scope & stream and first 5s invalid PTS when RTSP connection initialized
        let gap = 60;
        assert_timestamp_approx_eq("first_pts_read", first_pts_read, expected_timestamp, 0 * SECOND, gap * SECOND);
        assert_timestamp_approx_eq("last_pts_read", last_pts_read, expected_timestamp + (num_sec_to_record + num_sec_to_failure) * SECOND, 0 * SECOND, gap * 2 * SECOND);
        assert!(summary_read.pts_range() >= (num_sec_to_record + num_sec_to_failure - 10 * 2) * SECOND);
        assert!(summary_read.pts_range() <= (num_sec_to_record + num_sec_to_failure + gap) * SECOND);

        info!("#### Read recorded stream from Pravega, complete decoding, part 2");
        let pipeline_description_decode = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
              end-mode=latest \
            ! identity name=fromsource silent=false \
            ! decodebin \
            ! identity name=decoded silent=false \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_decoded = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
        summary_decoded.dump("summary_decoded: ");
        debug!("summary_decoded={}", summary_decoded);
        let first_pts_decoded = summary_decoded.first_valid_pts();
        let last_pts_decoded = summary_decoded.last_valid_pts();
        assert!(first_pts_decoded.is_some(), "Pipeline is not recording timestamps");
        assert_between_u64("decreasing_pts_count", summary_decoded.decreasing_pts_count(), 0, 0);
        let decode_margin = 10 * SECOND;
        assert_timestamp_approx_eq("first_pts_decoded", first_pts_decoded, first_pts_read, decode_margin, decode_margin);
        assert_timestamp_approx_eq("last_pts_decoded", last_pts_decoded, last_pts_read, decode_margin, decode_margin);
        assert!(summary_decoded.pts_range() >= (num_sec_to_record + num_sec_to_failure - 10 * 2) * SECOND);
        assert!(summary_decoded.pts_range() <= (num_sec_to_record + num_sec_to_failure + gap) * SECOND);
        let num_frames_expected_min = (num_sec_to_record + num_sec_to_failure - 10 * 2) * fps;
        let num_frames_expected_max = (num_sec_to_record + num_sec_to_failure) * fps;
        assert_between_u64("num_buffers", summary_decoded.num_buffers(), num_frames_expected_min, num_frames_expected_max);
        assert_between_u64("num_buffers_with_valid_pts", summary_decoded.num_buffers_with_valid_pts(), num_frames_expected_min, num_frames_expected_max);
        // Last 2 buffers are usually corrupted. These can be ignored.
        assert_between_u64("corrupted_buffer_count", summary_decoded.corrupted_buffer_count(), 0, 2);
        assert_between_u64("imperfect_timestamp_count", summary_decoded.imperfect_pts_count(TimeDelta::none()), 0, 0);
    }
}

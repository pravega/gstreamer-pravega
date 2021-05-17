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
    use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, HOUR, MSECOND, SECOND};
    use rstest::rstest;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::{start_or_get_rtsp_test_source, RTSPCameraSimulatorConfig, RTSPCameraSimulatorConfigBuilder};
    use crate::utils::*;

    #[rstest]
    #[case("none", true, "running-time", Realtime)]
    fn test_rtspsrc_ignore(#[case] buffer_mode: &str, #[case] ntp_sync: bool, #[case] ntp_time_source: &str, #[case] clock_type: gst::ClockType) {
        gst_init();
        let clock = gst::SystemClock::obtain();
        clock.set_property("clock-type", &clock_type).unwrap();
        gst::SystemClock::set_default(Some(&clock));
        let rtsp_server_config = RTSPCameraSimulatorConfigBuilder::default().fps(20).build().unwrap();
        let (rtsp_url, _rtsp_server) = start_or_get_rtsp_test_source(rtsp_server_config);
        info!("### BEGIN: buffer_mode={}, ntp_sync={}, ntp_time_source={}, clock_type={:?}",
            buffer_mode, ntp_sync, ntp_time_source, clock_type);
        let num_buffers = 5*60*20;
        let pipeline_description = format!("\
            rtspsrc \
              buffer-mode={buffer_mode} \
              drop-on-latency=true \
              latency=2000 \
              location={rtsp_url} \
              ntp-sync={ntp_sync} \
              ntp-time-source={ntp_time_source} \
            ! identity name=rtspsrc silent=false \
            ! application/x-rtp,media=video \
            ! rtph264depay \
            ! identity name=depay__ silent=true \
            ! h264parse \
            ! video/x-h264,alignment=au \
            ! identity name=h264par silent=false eos-after={num_buffers} \
            ! timestampcvt \
            ! identity name=tscvt__ silent=false \
            ! appsink name=sink sync=false \
            ",
            rtsp_url = rtsp_url,
            num_buffers = num_buffers,
            buffer_mode = buffer_mode,
            ntp_sync = ntp_sync,
            ntp_time_source = ntp_time_source,
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        let have_dts = summary.first_valid_dts().is_some();
        summary.dump_timestamps("summary: ");
        info!("### SUMMARY: buffer_mode={}, ntp_sync={}, ntp_time_source={}, clock_type={:?}, have_dts={}",
            buffer_mode, ntp_sync, ntp_time_source, clock_type, have_dts);
        debug!("summary={}", summary);
        assert_between_u64("decreasing_pts_count", summary.decreasing_pts_count(), 0, 0);
        assert_between_u64("decreasing_dts_count", summary.decreasing_dts_count(), 0, 0);
        assert_between_u64("corrupted_buffer_count", summary.corrupted_buffer_count(), 0, 0);
        info!("### END");
    }

    #[rstest]
    #[case(
        RTSPCameraSimulatorConfigBuilder::default().fps(20).build().unwrap(),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
        3*60,
        true,
    )]
    // TODO: Below disabled because fragments with more than 1 frame result in corruption with real RTSP camera.
    //       Workaround is to use fragment duration 1 ms which is tested above.
    // #[case(
    //     RTSPCameraSimulatorConfigBuilder::default().fps(20).build().unwrap(),
    //     ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(100 * MSECOND).build().unwrap()),
    // )]
    // TODO: Below disabled because PTS test fails with B-frames.
    // #[case(
    //     RTSPCameraSimulatorConfigBuilder::default().fps(20).tune("0".to_owned()).build().unwrap(),
    //     ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    //     20,
    //     true
    // )]
    // TODO: Below disabled because MPEG TS fails imperfect_timestamp_count test..
    // #[case(
    //     RTSPCameraSimulatorConfigBuilder::default().fps(20).build().unwrap(),
    //     ContainerFormat::MpegTs,
    //     20,
    //     true
    // )]
    fn test_rtsp(#[case] rtsp_server_config: RTSPCameraSimulatorConfig, #[case] container_format: ContainerFormat,
            #[case] num_sec_to_record: u64, #[case] restart: bool) {
        gst_init();
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-rtsp-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let container_pipeline = container_format.pipeline();
        let demux_pipeline = match container_format {
            ContainerFormat::Mp4(_) => format!("qtdemux"),
            ContainerFormat::MpegTs => format!("tsdemux"),
        };

        let fps = rtsp_server_config.fps;
        let (rtsp_url, _rtsp_server) = start_or_get_rtsp_test_source(rtsp_server_config);
        // The identity element will stop the pipeline after this many video frames.
        let num_frames_to_record = num_sec_to_record * fps;
        let num_sec_expected_min = num_sec_to_record - 50;
        let num_frames_expected_min = num_sec_expected_min * fps;
        let num_frames_expected_max = num_frames_to_record;
        // We expect the RTSP camera's clock to be within 24 hours of this computer's clock.
        let expected_timestamp = PravegaTimestamp::now();
        let expected_timestamp_margin = 24 * HOUR;
        let frame_duration = 1 * SECOND / fps;

        info!("#### Record RTSP camera to Pravega, part 1");
        // TODO: Test with queue?: queue max-size-buffers=0 max-size-bytes=10485760 max-size-time=0 silent=true
        let pipeline_description_record = format!("\
            rtspsrc \
              buffer-mode=none \
              drop-on-latency=true \
              latency=2000 \
              location={rtsp_url} \
              ntp-sync=true \
              ntp-time-source=running-time \
            ! identity name=rtspsrc silent=false \
            ! application/x-rtp,media=video \
            ! rtph264depay \
            ! identity name=depay__ silent=false \
            ! h264parse \
            ! video/x-h264,alignment=au \
            ! identity name=h264par silent=false eos-after={num_frames} \
            ! timestampcvt \
            ! identity name=tscvt__ silent=false \
            ! {container_pipeline} \
            ! pravegasink {pravega_plugin_properties} \
              sync=false \
              timestamp-mode=tai \
            ",
           pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
           rtsp_url = rtsp_url,
           num_frames = num_frames_to_record,
           container_pipeline = container_pipeline,
        );
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

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
        assert_timestamp_approx_eq("first_pts_read", first_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert_timestamp_approx_eq("last_pts_read", last_pts_read, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert!(summary_read.pts_range() >= num_sec_expected_min * SECOND);
        assert!(summary_read.pts_range() <= (2 * num_sec_to_record + 60) * SECOND);

        info!("#### Read recorded stream from Pravega, demux, no decoding, part 1");
        let pipeline_description_demux = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
              end-mode=latest \
            ! {demux_pipeline} \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            demux_pipeline = demux_pipeline,
        );
        let summary_demux = launch_pipeline_and_get_summary(&pipeline_description_demux).unwrap();
        debug!("summary_demux={}", summary_demux);
        let first_pts_demux = summary_demux.first_valid_pts();
        let last_pts_demux = summary_demux.last_valid_pts();
        assert!(first_pts_demux.is_some(), "Pipeline is not recording timestamps");
        assert_between_u64("decreasing_pts_count", summary_demux.decreasing_pts_count(), 0, 0);
        assert_between_u64("decreasing_dts_count", summary_demux.decreasing_dts_count(), 0, 0);
        assert_timestamp_approx_eq("first_pts_demux", first_pts_demux, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert_timestamp_approx_eq("last_pts_demux", last_pts_demux, expected_timestamp, expected_timestamp_margin, expected_timestamp_margin);
        assert!(summary_demux.pts_range() >= num_sec_expected_min * SECOND);
        assert!(summary_demux.pts_range() <= (2 * num_sec_to_record + 60) * SECOND);
        assert_between_u64("corrupted_buffer_count", summary_demux.corrupted_buffer_count(), 0, 0);
        assert_between_u64("imperfect_timestamp_count", summary_demux.imperfect_pts_count(TimeDelta::none()), 0, 0);

        info!("#### Read recorded stream from Pravega, complete decoding, part 1");
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
        debug!("summary_read   ={}", summary_read);
        debug!("summary_decoded={}", summary_decoded);
        let first_pts_decoded = summary_decoded.first_valid_pts();
        let last_pts_decoded = summary_decoded.last_valid_pts();
        assert!(first_pts_decoded.is_some(), "Pipeline is not recording timestamps");
        assert_between_u64("decreasing_pts_count", summary_demux.decreasing_pts_count(), 0, 0);
        let decode_margin = 10 * SECOND;
        assert_timestamp_approx_eq("first_pts_decoded", first_pts_decoded, first_pts_read, decode_margin, decode_margin);
        assert_timestamp_approx_eq("last_pts_decoded", last_pts_decoded, last_pts_read, decode_margin, decode_margin);
        assert!(summary_decoded.pts_range() >= num_sec_expected_min * SECOND);
        assert!(summary_decoded.pts_range() <= (2 * num_sec_to_record + 60) * SECOND);
        assert_between_u64("num_buffers", summary_decoded.num_buffers(), num_frames_expected_min, num_frames_expected_max);
        assert_between_u64("num_buffers_with_valid_pts", summary_decoded.num_buffers_with_valid_pts(), num_frames_expected_min, num_frames_expected_max);
        // Last 2 buffers are usually corrupted. These can be ignored.
        assert_between_u64("corrupted_buffer_count", summary_decoded.corrupted_buffer_count(), 0, 2);
        assert_between_u64("imperfect_timestamp_count", summary_decoded.imperfect_pts_count(TimeDelta::none()), 0, 0);

        // Simulate restart of recorder.
        if restart {
            info!("#### Record RTSP camera to Pravega, part 2");
            let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

            info!("#### Read recorded stream from Pravega, no demux, no decoding, part 2");
            let summary_read2 = launch_pipeline_and_get_summary(&pipeline_description_read).unwrap();
            debug!("summary_read ={}", summary_read);
            debug!("summary_read2={}", summary_read2);
            // summary_read2.dump("summary_read2: ");
            let first_pts_read2 = summary_read2.first_valid_pts();
            let last_pts_read2 = summary_read2.last_valid_pts();
            let max_gap_sec = 300;
            assert_timestamp_approx_eq("first_pts_read2", first_pts_read2, first_pts_read, 0 * SECOND, frame_duration/2);
            assert_timestamp_approx_eq("last_pts_read2", last_pts_read2, last_pts_read, 0 * SECOND,
                (2 * num_sec_to_record + max_gap_sec) * SECOND);
            assert!(summary_read2.pts_range() >= 2 * num_sec_expected_min * SECOND);
            assert!(summary_read2.pts_range() <= (4 * num_sec_to_record + max_gap_sec) * SECOND);

            info!("#### Read recorded stream from Pravega, complete decoding, part 2");
            let summary_decoded2 = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
            debug!("summary_decoded ={}", summary_decoded);
            debug!("summary_decoded2={}", summary_decoded2);
            let first_pts_decoded2 = summary_decoded2.first_valid_pts();
            let last_pts_decoded2 = summary_decoded2.last_valid_pts();
            assert_between_u64("decreasing_pts_count", summary_demux.decreasing_pts_count(), 0, 0);
            assert_timestamp_approx_eq("first_pts_decoded2", first_pts_decoded2, first_pts_read2, decode_margin, decode_margin);
            assert_timestamp_approx_eq("last_pts_decoded2", last_pts_decoded2, last_pts_read2, decode_margin, decode_margin);
            assert!(summary_decoded2.pts_range() >= 2 * num_sec_expected_min * SECOND);
            assert!(summary_decoded2.pts_range() <= (4 * num_sec_to_record + max_gap_sec) * SECOND);
            assert_between_u64("num_buffers", summary_decoded2.num_buffers(),
                2 * num_frames_expected_min, 2 * num_frames_expected_max);
            assert_between_u64("num_buffers_with_valid_pts", summary_decoded2.num_buffers_with_valid_pts(),
                2 * num_frames_expected_min, 2 * num_frames_expected_max);
            // TODO: Investigate why so many buffers are corrupted when restarting recording.
            assert_between_u64("corrupted_buffer_count", summary_decoded2.corrupted_buffer_count(), 0, 100);
        }

        let interactive = false;
        if interactive {
            info!("#### Play video stream from beginning on screen");
            info!("You should see the first part of the video play in a window, followed by a 10-30 second pause, then the next part will play.");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                  end-mode=latest \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            );
            launch_pipeline(&pipeline_description).unwrap();
        }
    }
}

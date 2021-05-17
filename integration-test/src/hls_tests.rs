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
    use pravega_video::timestamp::{PravegaTimestamp, MSECOND};
    use rstest::rstest;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::{start_or_get_rtsp_test_source, RTSPCameraSimulatorConfigBuilder};
    use crate::utils::*;

    /// Test playback using an HLS player.
    /// This is an interactive test.
    /// When executed, it will pause when it expects the user to open a browser to a URL and validate playback.
    /// To run this test:
    ///   1. scripts/pravega-video-server.sh
    ///   1. scripts/test-hls.sh
    #[rstest]
    #[case(ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()))]
    #[case(ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(100 * MSECOND).build().unwrap()))]
    #[case(ContainerFormat::MpegTs)]
    fn test_hls_rtsp_ignore(#[case] container_format: ContainerFormat) {
        gst_init();
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-hls-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let container_pipeline = container_format.pipeline();

        let player_url = format!("http://localhost:3030/player?scope={scope}&stream={stream_name}",
            scope = test_config.scope, stream_name = stream_name);
        info!("\n\nHLS player URL: {}\n", player_url);
        info!("Copy the above URL, press enter, then open the URL in your browser. It should begin playing the video within 20 seconds.");
        let _ = std::io::stdin().read_line(&mut String::new());

        let num_sec_to_record = 60;
        let fps = 20;
        let num_frames_to_record = num_sec_to_record * fps;
        let rtsp_server_config = RTSPCameraSimulatorConfigBuilder::default()
            .width(320)
            .height(200)
            .fps(fps)
            .key_frame_interval_max(fps)
            .target_rate_kilobytes_per_sec(20.0)
            .build().unwrap();
        let (rtsp_url, _rtsp_server) = start_or_get_rtsp_test_source(rtsp_server_config);

        info!("#### Record RTSP camera to Pravega, part 1");
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
            ! timestampcvt \
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

        info!("#### Read recorded stream from Pravega without decoding, part 1");
        let pipeline_description_decode = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
              end-mode=latest \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary1 = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
        debug!("summary1={}", summary1);
        let first_pts1 = summary1.first_valid_pts();
        let last_pts1 = summary1.last_valid_pts();

        info!("\n\nHLS player URL: {}\n", player_url);
        info!("Confirm historical playback of part 1. Press enter to continue.");
        let _ = std::io::stdin().read_line(&mut String::new());

        // Simulate restart of recorder.
        info!("#### Record RTSP camera to Pravega, part 2");
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

        info!("#### Read recorded stream from Pravega without decoding, part 2");
        let summary2 = launch_pipeline_and_get_summary(&pipeline_description_decode).unwrap();
        debug!("summary1={}", summary1);
        debug!("summary2={}", summary2);
        let last_pts2= summary2.last_valid_pts();
        let first_buffer_after_part_1 = summary2.first_buffer_after(last_pts1).unwrap();
        debug!("first_buffer_after_part_1={:?}", first_buffer_after_part_1);
        let first_pts_after_part_1 = first_buffer_after_part_1.pts;
        assert_between_timestamp("first_pts_after_part_1", first_pts_after_part_1, last_pts1, PravegaTimestamp::none());

        info!("\n\n\
            HLS player URL (all):    {player_url}\n\
            HLS player URL (part 1): {player_url}&begin={first_pts1}&end={last_pts1}\n\
            HLS player URL (part 2): {player_url}&begin={first_pts_after_part_1}&end={last_pts2}\n\
            ",
            player_url = player_url,
            first_pts1 = first_pts1,
            last_pts1 = last_pts1,
            first_pts_after_part_1 = first_pts_after_part_1,
            last_pts2 = last_pts2,
        );
        info!("Confirm historical playback. There will be a gap at around {} seconds. Press enter to continue.", num_sec_to_record);
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

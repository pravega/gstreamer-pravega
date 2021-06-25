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
    use anyhow::Error;
    use gst::prelude::*;
    use gstpravega::utils::{clocktime_to_pravega, pravega_to_clocktime};
    use pravega_video::timestamp::{PravegaTimestamp, MSECOND, NSECOND, SECOND};
    use rstest::rstest;
    use std::convert::TryFrom;
    use std::sync::Arc;
    use std::env;
    use std::time::Instant;
    #[allow(unused_imports)]
    use tracing::{error, info, debug, trace};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    fn failure_recovery_test_data_gen(test_config: &TestConfig, stream_name: &str, video_encoder: VideoEncoder,
        container_format: ContainerFormat, length_sec: i32) -> Result<BufferListSummary, Error> {
        gst_init();
        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={:?}", first_timestamp);
        let fps = 30;
        let num_buffers_written = length_sec * fps;
        let video_encoder_pipeline = video_encoder.pipeline();
        let container_pipeline = container_format.pipeline();

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! {video_encoder_pipeline} \
            ! {container_pipeline} \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_timestamp.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={}", summary);
        Ok(summary)
    }

    /// This will test starting a decode pipeline at a precise time.
    /// The pipeline should start decoding at the random access point prior to the specified timestamp
    /// but the decoder should not emit frames earlier than the specified timestamp.
    /// See also test_pravegasrc_start_mode_timestamp().
    #[rstest]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(60).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(500 * MSECOND).build().unwrap()),
    )]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).tune("0".to_owned()).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(200 * MSECOND).build().unwrap()),
    )]
    fn test_pravegasrc_decode_from_timestamp(#[case] video_encoder: VideoEncoder, #[case] container_format: ContainerFormat) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegasrc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = failure_recovery_test_data_gen(test_config, stream_name, video_encoder, container_format, 60).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();
        let last_pts_written = summary_written.last_valid_pts();

        info!("#### Decode entire video stream");
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=earliest \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_full = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written={}", summary_written);
        debug!("summary_full=   {}", summary_full);
        let first_pts_full = summary_full.first_valid_pts();
        let last_pts_full = summary_full.last_valid_pts();

        info!("#### Decode video stream starting from exact PTS");
        let resume_from_pts: PravegaTimestamp = first_pts_full + 30510 * MSECOND;
        let pipeline_description = format!("\
            pravegasrc {pravega_plugin_properties} \
              start-mode=timestamp \
              start-timestamp={resume_from_pts} \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            resume_from_pts = resume_from_pts.nanoseconds().unwrap(),
        );

        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written={}", summary_written);
        debug!("summary_full=   {}", summary_full);
        debug!("summary=        {}", summary);

        let first_pts_read = summary.first_valid_pts();
        let last_pts_read = summary.last_valid_pts();
        debug!("first_pts_written={:?}", first_pts_written);
        debug!("first_pts_full=   {:?}", first_pts_full);
        debug!("resume_from_pts=  {:?}", resume_from_pts);
        debug!("first_pts_read=   {:?}", first_pts_read);
        debug!("last_pts_written= {:?}", last_pts_written);
        debug!("last_pts_full=    {:?}", last_pts_full);
        debug!("last_pts_read=    {:?}", last_pts_read);
        assert_timestamp_eq("first_pts_read", first_pts_read, resume_from_pts);
        assert_timestamp_eq("last_pts_read", last_pts_read, last_pts_full);
    }

    #[rstest]
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::Mp4(Mp4MuxConfigBuilder::default().fragment_duration(1 * MSECOND).build().unwrap()),
    )]
    fn test_transaction_coordinator_1(#[case] video_encoder: VideoEncoder, #[case] container_format: ContainerFormat) {
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-pravegatc-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let table_name = &format!("test-pravegatc-table-{}-{}", test_config.test_id, Uuid::new_v4())[..];
        let summary_written = failure_recovery_test_data_gen(test_config, stream_name, video_encoder, container_format, 10).unwrap();
        debug!("summary_written={}", summary_written);
        let first_pts_written = summary_written.first_valid_pts();

        info!("#### Decode entire video stream - without pravegatc");
        let pipeline_description = format!("\
            pravegasrc name=src {pravega_plugin_properties} \
              start-mode=earliest \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_without_pravegatc = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        let first_pts_without_pravegatc = summary_without_pravegatc.first_pts();
        let last_pts_without_pravegatc = summary_without_pravegatc.last_pts();
        debug!("summary_written=          {}", summary_written);
        debug!("summary_without_pravegatc={}", summary_without_pravegatc);

        info!("#### Decode entire video stream - run 1, with injected fault");
        let pipeline_description = format!("\
            pravegasrc name=src {pravega_plugin_properties} \
              start-mode=earliest \
            ! identity name=before_decode silent=false \
            ! decodebin \
            ! identity name=after_decode silent=false \
            ! pravegatc name=pravegatc controller={controller_uri} table={scope}/{table_name} \
            ! appsink name=sink \
              sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            controller_uri = test_config.client_config.clone().controller_uri.0,
            scope = test_config.scope,
            table_name = table_name,
        );
        let fault_injection_pts: PravegaTimestamp = first_pts_written + 5510 * MSECOND;
        env::set_var("FAULT_INJECTION_PTS_pravegatc", format!("{}", fault_injection_pts.nanoseconds().unwrap()));
        let summary_run1 = launch_pipeline_and_get_summary(&pipeline_description);
        let summary_run1 = match summary_run1 {
            Ok(_) => panic!("Error expected"),
            Err(LaunchPipelineError { error, buffer_list_summary}) => {
                debug!("Expected error: {}", error);
                buffer_list_summary
            },
        };
        debug!("summary_written=          {}", summary_written);
        debug!("summary_without_pravegatc={}", summary_without_pravegatc);
        debug!("summary_run1=             {}", summary_run1);
        debug!("fault_injection_pts={:?}", fault_injection_pts);
        let first_pts_run1 = summary_run1.first_pts();
        let last_pts_run1 = summary_run1.last_pts();
        assert_timestamp_eq("first_pts_run1", first_pts_run1, first_pts_without_pravegatc);
        assert_between_timestamp("last_pts_run1", last_pts_run1, fault_injection_pts - 34 * MSECOND, fault_injection_pts - 1 * NSECOND);

        info!("#### Decode entire video stream - run 2, resume from fault");
        env::remove_var("FAULT_INJECTION_PTS_pravegatc");
        let summary_run2 = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written=          {}", summary_written);
        debug!("summary_without_pravegatc={}", summary_without_pravegatc);
        debug!("summary_run1=             {}", summary_run1);
        debug!("summary_run2=             {}", summary_run2);
        let first_pts_run2 = summary_run2.first_pts();
        let last_pts_run2 = summary_run2.last_pts();
        assert_between_timestamp("first_pts_run2", first_pts_run2, last_pts_run1 + 1 * NSECOND, last_pts_run1 + 34 * MSECOND);
        assert_timestamp_eq("last_pts_run2", last_pts_run2, last_pts_without_pravegatc);
    }
}

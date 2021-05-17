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
    use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, SECOND, MSECOND};
    use rstest::rstest;
    use std::convert::TryFrom;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    /// Test truncation with raw video (uncompressed).
    /// This avoids any complexities caused by video encoding and decoding.
    #[test]
    fn test_raw_video() {
        gst_init();
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-raw-video-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_pts_written = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_pts_written={}", first_pts_written);
        let fps = 30;
        let length_sec = 5;
        let num_buffers_written = length_sec * fps;

        info!("#### Write video stream to Pravega");
        // Since raw video does not have delta frames, we force an index record every 1 second.
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=100,height=100,framerate={fps}/1 \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false index-min-sec=1.0",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
        );
        let summary_written = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary_written={:?}", summary_written);

        info!("#### Read video stream from beginning");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let last_pts_written = summary.last_pts();
        assert_eq!(summary, summary_written);

        info!("#### Truncate stream");
        let truncate_sec = 1;
        let truncate_before_pts = first_pts_written + truncate_sec * SECOND;
        truncate_stream(test_config.client_config.clone(), test_config.scope.clone(), stream_name.to_owned(), truncate_before_pts);

        info!("#### Read video from truncated position");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let num_buffers_actual = summary.num_buffers();
        let first_pts_actual = summary.first_pts();
        let last_pts_actual = summary.last_pts();
        let first_pts_expected = truncate_before_pts;
        let num_buffers_expected = num_buffers_written - truncate_sec * fps;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_expected, first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        assert_between_timestamp("first_pts_actual", first_pts_actual, first_pts_expected, first_pts_expected);
        assert_between_timestamp("last_pts_actual", last_pts_actual, last_pts_written, last_pts_written);
        assert_eq!(num_buffers_actual, num_buffers_expected);

        info!("#### END");
    }

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
    #[case(
        VideoEncoder::H264(H264EncoderConfigBuilder::default().key_int_max_frames(30).build().unwrap()),
        ContainerFormat::MpegTs,
    )]
    fn test_compressed_video(#[case] video_encoder: VideoEncoder, #[case] container_format: ContainerFormat) {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        gst_init();
        let stream_name = &format!("test-compressed-video-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let video_encoder_pipeline = video_encoder.pipeline();
        let container_pipeline = container_format.pipeline();
        let (pts_margin, random_start_pts_margin) = match container_format {
            // MPEG TS requires large margins to pass tests.
            ContainerFormat::MpegTs => (126 * MSECOND, 1000 * MSECOND),
            ContainerFormat::Mp4(_) => (0 * MSECOND, 0 * MSECOND),
        };

        // first_pts_written: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_pts_written = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_pts_written={}", first_pts_written);
        let fps = 30;
        let length_sec = 10;
        let num_buffers_written = length_sec * fps;
        let key_int_max_frames = match video_encoder {
            VideoEncoder::H264(config) => config.key_int_max_frames,
        };
        let key_int_max_time_delta: TimeDelta = key_int_max_frames * SECOND / fps;
        let last_pts_written = first_pts_written + (num_buffers_written - 1) * SECOND / fps;
        info!("last_pts_written={}", last_pts_written);

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! timeoverlay valignment=bottom font-desc=\"Sans 48px\" shaded-background=true \
            ! videoconvert \
            ! {video_encoder_pipeline} \
            ! identity name=h264___ silent=false \
            ! {container_pipeline} \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            video_encoder_pipeline = video_encoder_pipeline,
            container_pipeline = container_pipeline,
        );
        let summary_written = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_written.dump("summary_written");
        debug!("summary_written={}", summary_written);

        info!("#### Read video stream from beginning with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! decodebin \
            ! identity silent=false \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_full = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_full.dump("summary_full");
        debug!("summary_full={}", summary_full);
        let num_buffers_actual = summary_full.num_buffers();
        let first_pts_actual = summary_full.first_pts();
        let last_pts_actual = summary_full.last_pts();
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_written, 0 * SECOND, pts_margin);
        assert_timestamp_approx_eq("last_pts_actual", last_pts_actual, last_pts_written, 0 * SECOND, pts_margin);
        assert_eq!(num_buffers_actual, num_buffers_written as u64);
        assert_between_u64("corrupted_buffer_count", summary_full.corrupted_buffer_count(), 0, 2);

        if false {
            info!("#### Play video stream from beginning on screen");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            );
            launch_pipeline(&pipeline_description).unwrap();
        }

        info!("#### Truncate stream");
        let truncate_time_delta = key_int_max_time_delta;
        let truncate_frames: Option<i128> = fps * truncate_time_delta / SECOND;
        let truncate_frames = truncate_frames.unwrap() as u64;
        let truncate_before_pts = first_pts_written + truncate_time_delta;
        info!("first_pts_written=  {:?}", first_pts_written);
        info!("truncate_before_pts={:?}", truncate_before_pts);
        truncate_stream(test_config.client_config.clone(), test_config.scope.clone(), stream_name.to_owned(), truncate_before_pts);

        info!("#### Read video from truncated position without decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_trun_read = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_trun_read.dump("summary_trun_read");
        debug!("summary_trun_read={}", summary_trun_read);
        let first_pts_actual = summary_trun_read.first_pts();
        let last_pts_actual = summary_trun_read.last_pts();
        let first_pts_expected = truncate_before_pts;
        let last_pts_expected_min = last_pts_written - key_int_max_time_delta;
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_expected, pts_margin, pts_margin);
        assert_between_timestamp("last_pts_actual", last_pts_actual, last_pts_expected_min - pts_margin, last_pts_written + pts_margin);

        if false {
            info!("#### Play video stream from truncated position on screen");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            );
            launch_pipeline(&pipeline_description).unwrap();
        }

        info!("#### Read video from truncated position with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
            ! identity name=src_____ silent=false \
            ! decodebin \
            ! identity name=decoded silent=false \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary_trunc_decoded = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_trunc_decoded.dump("summary_trunc_decoded");
        debug!("summary_trunc_decoded={}", summary_trunc_decoded);
        let num_buffers_actual = summary_trunc_decoded.num_buffers();
        let first_pts_actual = summary_trunc_decoded.first_pts();
        let last_pts_actual = summary_trunc_decoded.last_pts();
        let first_pts_expected = truncate_before_pts;
        let num_buffers_expected = num_buffers_written - truncate_frames;
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_expected,
                                   pts_margin, pts_margin + random_start_pts_margin);
        assert_timestamp_approx_eq("last_pts_actual", last_pts_actual, last_pts_written,
                                   0 * SECOND, pts_margin);
        assert_between_u64("num_buffers_actual", num_buffers_actual, num_buffers_expected - fps, num_buffers_expected);
        assert_between_u64("corrupted_buffer_count", summary_trunc_decoded.corrupted_buffer_count(), 0, 2);

        info!("#### END");
    }
}

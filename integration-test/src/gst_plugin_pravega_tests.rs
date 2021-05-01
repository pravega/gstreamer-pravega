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
    use gst::ClockTime;
    use gst::prelude::*;
    use gstpravega::utils::{clocktime_to_pravega, pravega_to_clocktime};
    use pravega_video::timestamp::PravegaTimestamp;
    use rstest::rstest;
    use std::convert::TryFrom;
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    /// Test pravegasink and pravegasrc with raw video (uncompressed).
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
        let summary_written = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary_written={:?}", summary_written);

        info!("#### Read video stream from beginning");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let last_pts_written = summary.last_pts();
        assert_eq!(summary, summary_written);

        info!("#### Truncate stream");
        let truncate_sec = 1;
        let truncate_before_pts = clocktime_to_pravega(pravega_to_clocktime(first_pts_written) + truncate_sec * gst::SECOND);
        truncate_stream(test_config.client_config.clone(), test_config.scope.clone(), stream_name.to_owned(), truncate_before_pts);

        info!("#### Read video from truncated position");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
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

    #[test]
    fn test_mpeg_ts_video() {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        let compression_pipeline = format!(
            "x264enc key-int-max=30 bitrate=100 \
            ! mpegtsmux",
        );
        let pts_margin = 126 * gst::MSECOND;
        let random_start_pts_margin = 1000 * gst::MSECOND;
        test_compressed_video(test_config, compression_pipeline, pts_margin, random_start_pts_margin)
    }

    // TODO: MP4 support is not yet working. See scripts/mp4-test*.sh.
    #[test]
    fn test_mp4_video() {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        let compression_pipeline = format!(
            "x264enc key-int-max=30 bitrate=100 \
            ! mp4mux streamable=true fragment-duration=100",
        );
        let pts_margin = 126 * gst::MSECOND;
        let random_start_pts_margin = 1000 * gst::MSECOND;
        test_compressed_video(test_config, compression_pipeline, pts_margin, random_start_pts_margin)
    }

    fn test_compressed_video(test_config: TestConfig, compression_pipeline: String, pts_margin: ClockTime, random_start_pts_margin:ClockTime) {
        gst_init();
        let stream_name = &format!("test-compressed-video-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_pts_written = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_pts_written={}", first_pts_written);
        let fps = 30;
        let length_sec = 10;
        let num_buffers_written = length_sec * fps;
        let last_pts_written = clocktime_to_pravega(pravega_to_clocktime(first_pts_written) + (num_buffers_written - 1) * gst::SECOND / fps);
        info!("last_pts_written={}", last_pts_written);

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! timeoverlay valignment=bottom font-desc=\"Sans 48px\" shaded-background=true \
            ! videoconvert \
            ! {compression_pipeline} \
            ! tee name=t \
            t. ! queue ! appsink name=sink sync=false \
            t. ! pravegasink {pravega_plugin_properties} \
                 seal=true timestamp-mode=tai sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            compression_pipeline = compression_pipeline,
        );
        let summary_written = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary_written={:?}", summary_written);

        info!("#### Read video stream from beginning");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
              start-mode=no-seek \
            ! decodebin \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let num_buffers_actual = summary.num_buffers();
        let first_pts_actual = summary.first_pts();
        let last_pts_actual = summary.last_pts();
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_written, first_pts_written, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_written, ClockTime::zero(), pts_margin);
        assert_timestamp_approx_eq("last_pts_actual", last_pts_actual, last_pts_written, ClockTime::zero(), pts_margin);
        assert_eq!(num_buffers_actual, num_buffers_written);

        if false {
            info!("#### Play video stream from beginning on screen");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
                timestamp_offset = -1 * (first_pts_written.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(pipeline_description).unwrap();
        }

        info!("#### Truncate stream");
        let truncate_sec = 1;
        let truncate_before_pts = clocktime_to_pravega(pravega_to_clocktime(first_pts_written) + truncate_sec * gst::SECOND);
        truncate_stream(test_config.client_config.clone(), test_config.scope.clone(), stream_name.to_owned(), truncate_before_pts);

        info!("#### Read video from truncated position without decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let num_buffers_actual = summary.num_buffers();
        let first_pts_actual = summary.first_pts();
        let last_pts_actual = summary.last_pts();
        let first_pts_expected = truncate_before_pts;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", "??", first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_expected, pts_margin, pts_margin);
        assert_timestamp_approx_eq("last_pts_actual", last_pts_actual, last_pts_written, pts_margin, pts_margin);

        if false {
            info!("#### Play video stream from truncated position on screen");
            let pipeline_description = format!(
                "pravegasrc {pravega_plugin_properties} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
                timestamp_offset = -1 * (first_pts_written.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(pipeline_description).unwrap();
        }

        info!("#### Read video from truncated position with decoding");
        let pipeline_description = format!(
            "pravegasrc {pravega_plugin_properties} \
            ! decodebin \
            ! appsink name=sink sync=false",
            pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
        );
        let summary = launch_pipeline_and_get_summary(pipeline_description).unwrap();
        debug!("summary={:?}", summary);
        let num_buffers_actual = summary.num_buffers();
        let first_pts_actual = summary.first_pts();
        let last_pts_actual = summary.last_pts();
        let first_pts_expected = truncate_before_pts;
        let num_buffers_expected = num_buffers_written - truncate_sec * fps;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_expected, first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        // Note that first pts may be off by 1 second. This is probably caused by missing MPEG TS initialization packets at precise start.
        assert_timestamp_approx_eq("first_pts_actual", first_pts_actual, first_pts_expected,
                                   pts_margin, pts_margin + random_start_pts_margin);
        assert_timestamp_approx_eq("last_pts_actual", last_pts_actual, last_pts_written,
                                   ClockTime::zero(), pts_margin);
        assert_between_u64("num_buffers_actual", num_buffers_actual, num_buffers_expected - fps, num_buffers_expected);

        info!("#### END");
    }
}

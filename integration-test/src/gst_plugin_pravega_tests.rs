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
    use pravega_video::timestamp::PravegaTimestamp;
    // use rstest::{fixture, rstest};
    use std::convert::TryFrom;
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::utils::*;

    /// Test pravegasink and pravegasrc with raw video (uncompressed).
    /// This avoids any complexities caused by video encoding and decoding.
    #[test]
    fn test_raw_video() {
        let test_config = get_test_config();
        info!("test_config={:?}", test_config);
        let controller_uri = test_config.client_config.clone().controller_uri.0;
        let scope = test_config.scope.clone();
        let stream_name = format!("test-raw-video-{}-{}", test_config.test_id, Uuid::new_v4());

        // Initialize GStreamer
        std::env::set_var("GST_DEBUG", "pravegasrc:LOG,pravegasink:LOG,basesink:INFO");
        gst::init().unwrap();
        gstpravega::plugin_register_static().unwrap();

        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={}", first_timestamp);
        let first_pts_written = ClockTime(first_timestamp.nanoseconds());
        info!("first_pts_written={}", first_pts_written);
        let fps = 30;
        let num_buffers_written = 3 * fps;
        let last_pts_written = first_pts_written + (num_buffers_written - 1) * gst::SECOND / fps;
        info!("last_pts_written={}", last_pts_written);

        info!("#### Write video stream to Pravega");
        // Since raw video does not have delta frames, we force an index record every 1 second.
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=100,height=100,framerate={fps}/1 \
            ! pravegasink controller={controller_uri} stream={scope}/{stream_name} \
            seal=true timestamp-mode=tai sync=false index-min-sec=1.0",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
        );
        launch_pipeline(pipeline_description).unwrap();

        info!("#### Read video stream from beginning");
        let pipeline_description = format!(
            "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
            ! appsink name=sink sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
        );
        let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
        debug!("read_pts={:?}", read_pts);
        let num_buffers_actual = read_pts.len() as u64;
        let first_pts_actual = read_pts[0];
        let last_pts_actual = *read_pts.last().unwrap();
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_written, first_pts_written, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        assert_between_clocktime("first_pts_actual", first_pts_actual, first_pts_written, first_pts_written);
        assert_between_clocktime("last_pts_actual", last_pts_actual, last_pts_written, last_pts_written);
        assert_eq!(num_buffers_actual, num_buffers_written);

        info!("#### Truncate at 1 second");
        let truncate_sec = 1;
        let truncate_before_pts = first_pts_written + truncate_sec * gst::SECOND;
        let truncate_before_timestamp = PravegaTimestamp::from_nanoseconds((truncate_before_pts).nanoseconds());
        truncate_stream(test_config.client_config, scope.clone(), stream_name.clone(), truncate_before_timestamp);

        info!("#### Read video from truncated position");
        let pipeline_description = format!(
            "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
            ! appsink name=sink sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
        );
        let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
        debug!("read_pts={:?}", read_pts);
        let num_buffers_actual = read_pts.len() as u64;
        let first_pts_actual = read_pts[0];
        let last_pts_actual = *read_pts.last().unwrap();
        let first_pts_expected = truncate_before_pts;
        let num_buffers_expected = num_buffers_written - truncate_sec * fps;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_expected, first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        assert_between_clocktime("first_pts_actual", first_pts_actual, first_pts_expected, first_pts_expected);
        assert_between_clocktime("last_pts_actual", last_pts_actual, last_pts_written, last_pts_written);
        assert_eq!(num_buffers_actual, num_buffers_expected);

        // TODO: Test pravegasrc start-mode=timestamp start-timestamp={start_timestamp}

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

    fn test_compressed_video(test_config: TestConfig, compression_pipeline: String, pts_margin: ClockTime, random_start_pts_margin:ClockTime) {
        let controller_uri = test_config.client_config.clone().controller_uri.0;
        let scope = test_config.scope.clone();
        let stream_name = format!("test-compressed-video-{}-{}", test_config.test_id, Uuid::new_v4());

        // Initialize GStreamer
        std::env::set_var("GST_DEBUG", "pravegasrc:LOG,pravegasink:LOG,basesink:INFO");
        gst::init().unwrap();
        gstpravega::plugin_register_static().unwrap();

        // first_timestamp: 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
        let first_utc = "2001-02-03T04:00:00.000Z".to_owned();
        let first_timestamp = PravegaTimestamp::try_from(Some(first_utc)).unwrap();
        info!("first_timestamp={}", first_timestamp);
        let first_pts_written = ClockTime(first_timestamp.nanoseconds());
        info!("first_pts_written={}", first_pts_written);
        let fps = 30;
        let num_buffers_written = 10 * fps;
        let last_pts_written = first_pts_written + (num_buffers_written - 1) * gst::SECOND / fps;
        info!("last_pts_written={}", last_pts_written);

        // let compression_pipeline = format!(
        //     "x264enc key-int-max=30 bitrate=100 \
        //     ! mpegtsmux",
        // );
        // let pts_margin = 126 * gst::MSECOND;
        // let random_start_pts_margin = 1000 * gst::MSECOND;

        info!("#### Write video stream to Pravega");
        let pipeline_description = format!(
            "videotestsrc name=src timestamp-offset={timestamp_offset} num-buffers={num_buffers} \
            ! video/x-raw,width=320,height=180,framerate={fps}/1 \
            ! videoconvert \
            ! timeoverlay valignment=bottom font-desc=\"Sans 48px\" shaded-background=true \
            ! videoconvert \
            ! {compression_pipeline} \
            ! pravegasink controller={controller_uri} stream={scope}/{stream_name} \
            seal=true timestamp-mode=tai sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
            timestamp_offset = first_pts_written.nanoseconds().unwrap(),
            num_buffers = num_buffers_written,
            fps = fps,
            compression_pipeline = compression_pipeline,
        );
        launch_pipeline(pipeline_description).unwrap();

        info!("#### Read video stream from beginning");
        let pipeline_description = format!(
            "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
            ! decodebin \
            ! appsink name=sink sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
        );
        let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
        debug!("read_pts={:?}", read_pts);
        let num_buffers_actual = read_pts.len() as u64;
        let first_pts_actual = read_pts[0];
        let last_pts_actual = *read_pts.last().unwrap();
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_written, first_pts_written, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        assert_between_clocktime("first_pts_actual", first_pts_actual, first_pts_written, first_pts_written + pts_margin);
        assert_between_clocktime("last_pts_actual", last_pts_actual, last_pts_written, last_pts_written + pts_margin);
        assert_eq!(num_buffers_actual, num_buffers_written);

        if false {
            info!("#### Play video stream from beginning on screen");
            let pipeline_description = format!(
                "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                controller_uri = controller_uri,
                scope = scope.clone(),
                stream_name = stream_name.clone(),
                timestamp_offset = -1 * (first_pts_written.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(pipeline_description).unwrap();
        }

        info!("#### Truncate stream");
        let truncate_sec = 5;
        let truncate_before_pts = first_pts_written + truncate_sec * gst::SECOND;
        let truncate_before_timestamp = PravegaTimestamp::from_nanoseconds((truncate_before_pts).nanoseconds());
        truncate_stream(test_config.client_config, scope.clone(), stream_name.clone(), truncate_before_timestamp);

        info!("#### Read video from truncated position without decoding");
        let pipeline_description = format!(
            "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
            ! appsink name=sink sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
        );
        let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
        debug!("read_pts={:?}", read_pts);
        let num_buffers_actual = read_pts.len() as u64;
        let first_pts_actual = read_pts[0];
        let last_pts_actual = *read_pts.last().unwrap();
        let first_pts_expected = truncate_before_pts;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", "??", first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        assert_between_clocktime("first_pts_actual", first_pts_actual, first_pts_expected - pts_margin, first_pts_expected + pts_margin);
        assert_between_clocktime("last_pts_actual", last_pts_actual, last_pts_written - pts_margin, last_pts_written + pts_margin);

        if false {
            info!("#### Play video stream from truncated position on screen");
            let pipeline_description = format!(
                "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
                ! decodebin \
                ! videoconvert \
                ! autovideosink sync=true ts-offset={timestamp_offset}",
                controller_uri = controller_uri,
                scope = scope.clone(),
                stream_name = stream_name.clone(),
                timestamp_offset = -1 * (first_pts_written.nanoseconds().unwrap() as i64),
            );
            launch_pipeline(pipeline_description).unwrap();
        }

        info!("#### Read video from truncated position");
        let pipeline_description = format!(
            "pravegasrc controller={controller_uri} stream={scope}/{stream_name} \
            ! decodebin \
            ! appsink name=sink sync=false",
            controller_uri = controller_uri,
            scope = scope.clone(),
            stream_name = stream_name.clone(),
        );
        let read_pts = launch_pipeline_and_get_pts(pipeline_description).unwrap();
        debug!("read_pts={:?}", read_pts);
        let num_buffers_actual = read_pts.len() as u64;
        let first_pts_actual = read_pts[0];
        let last_pts_actual = *read_pts.last().unwrap();
        let first_pts_expected = truncate_before_pts;
        let num_buffers_expected = num_buffers_written - truncate_sec * fps;
        info!("Expected: num_buffers={}, first_pts={}, last_pts={}", num_buffers_expected, first_pts_expected, last_pts_written);
        info!("Actual:   num_buffers={}, first_pts={}, last_pts={}", num_buffers_actual, first_pts_actual, last_pts_actual);
        // TODO: Why is PTS is off by 125 ms?
        // Note that first pts may be off by 1 second. This is probably caused by missing MPEG TS initialization packets at precise start.
        assert_between_clocktime("first_pts_actual", first_pts_actual,
            first_pts_expected - pts_margin, first_pts_expected + pts_margin + random_start_pts_margin);
        assert_between_clocktime("last_pts_actual", last_pts_actual, last_pts_written, last_pts_written + pts_margin);
        assert_between_u64("num_buffers_actual", num_buffers_actual, num_buffers_expected - fps, num_buffers_expected);

        // TODO: Test pravegasrc start-mode=timestamp start-timestamp={start_timestamp}

        // TODO: Out-of-band: Play using HLS player.

        info!("#### END");
    }
}

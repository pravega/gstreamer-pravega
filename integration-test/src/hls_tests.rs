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
    // use pravega_video::timestamp::PravegaTimestamp;
    #[allow(unused_imports)]
    use tracing::{error, info, debug};
    use uuid::Uuid;
    use crate::*;
    use crate::rtsp_camera_simulator::{RTSPCameraSimulator, RTSPCameraSimulatorConfigBuilder};
    use crate::utils::*;

    #[test]
    fn test_hls() {
        gst_init();
        let test_config = &get_test_config();
        info!("test_config={:?}", test_config);
        let stream_name = &format!("test-hls-{}-{}", test_config.test_id, Uuid::new_v4())[..];

        let player_url = format!("http://localhost:3030/player?scope={scope}&stream={stream_name}",
            scope = test_config.scope, stream_name = stream_name);
        info!("\n\nHLS player URL: {}\n", player_url);

        info!("Copy the above URL, press enter, then open the URL in your browser. It should begin playing the video within 20 seconds.");
        let _ = std::io::stdin().read_line(&mut String::new());

        let num_sec_to_record = 60;
        let fps = 30;
        let num_frames_to_record = num_sec_to_record * fps;
        let rtsp_server_config = RTSPCameraSimulatorConfigBuilder::default()
            .width(320)
            .height(200)
            .fps(fps)
            .key_frame_interval_max(fps)
            .target_rate_kilobytes_per_sec(20.0)
            .build().unwrap();
        let mut rtsp_server = RTSPCameraSimulator::new(rtsp_server_config).unwrap();
        rtsp_server.start().unwrap();
        let rtsp_url = rtsp_server.get_url().unwrap();

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
            ! mpegtsmux \
            ! pravegasink {pravega_plugin_properties} \
              sync=false \
              timestamp-mode=ntp \
              index-min-sec=5.0 \
            ",
           pravega_plugin_properties = test_config.pravega_plugin_properties(stream_name),
           rtsp_url = rtsp_url,
           num_frames = num_frames_to_record,
        );
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

        info!("Confirm historical playback. Press enter to continue.");
        let _ = std::io::stdin().read_line(&mut String::new());

        // Simulate restart of recorder.
        info!("#### Record RTSP camera to Pravega, part 2");
        let _ = launch_pipeline_and_get_summary(&pipeline_description_record).unwrap();

        info!("Confirm historical playback. There will be a gap at around {} seconds. Press enter to continue.", num_sec_to_record);
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

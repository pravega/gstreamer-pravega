//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Obsolete.

console.log("pravega-web-player.js: BEGIN");

var player = videojs('my_video_1', {
    // liveui: true,
    // techOrder:  ["youtube", "html5"],
    preload: 'auto',
    // controls: video.player.controls,
    // autoplay: video.player.autoplay,
    fluid: true,
    controlBar: {
        children: [
            "playToggle",
            // "volumeMenuButton",
            "durationDisplay",
            "timeDivider",
            "currentTimeDisplay",
            "progressControl",
            "remainingTimeDisplay",
            "fullscreenToggle"
        ]
    }
});

// if (Hls.isSupported()) {
//     var video = document.getElementById('video');
//     var hls = new Hls();
//     hls.loadSource('playlist2.m3u8');
//     hls.attachMedia(video);
//     hls.on(Hls.Events.MANIFEST_PARSED, function() {
//         console.logged("parsed");
//         video.play();
//     });
//     hls.on(Hls.Events.FRAG_CHANGED, function(event,data) {
//         console.log('current dateTime ' + data.frag.programDateTime);
//     });
// }

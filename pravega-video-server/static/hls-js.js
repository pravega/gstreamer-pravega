// HLS Player for Pravega

var video = null;

// Milliseconds since epoch when the playlist starts.
// This represents the wall clock time when the player slider is all the way to the left.
var playStartMillisSinceEpoch = null;

function load_video() {
    var scope = document.getElementById("scope").innerHTML;
    var stream = document.getElementById("stream").innerHTML;

    var query = ""
    var begin = document.getElementById("begin").innerHTML;
    if (begin != "") {
        query = query + ((query == "") ? "?" : "&") + "begin=" + new Date(begin).toISOString();
    }
    var end = document.getElementById("end").innerHTML;
    if (end != "") {
        query = query + ((query == "") ? "?" : "&") + "end=" + new Date(end).toISOString();
    }

    var manifestUri = "/scopes/" + scope + "/streams/" + stream + "/m3u8" + query;
    console.log(manifestUri);

    if (Hls.isSupported()) {
        video = document.getElementById('video');
        var hls = new Hls();
        hls.on(Hls.Events.FRAG_CHANGED, function(event, data) {
            // Each time we get a new fragment, revise playStartMillisSinceEpoch.
            playStartMillisSinceEpoch = data.frag.programDateTime - data.frag.startPTS * 1000.0;
        });
        hls.loadSource(manifestUri);
        hls.attachMedia(video);
    }
}

function showWallClockTime() {
    try {
        if (playStartMillisSinceEpoch != null) {
            var timestampDate = new Date(playStartMillisSinceEpoch + video.currentTime * 1000.0);
            document.getElementById("timestamp").innerHTML = timestampDate.toISOString();
        }
    } catch (err) {
        console.log(err);
    }
    setTimeout(showWallClockTime, 100);
}

load_video();
showWallClockTime();

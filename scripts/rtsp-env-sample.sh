# Sample environment variables for an RTSP camera.
export CAMERA_USER=admin
export CAMERA_IP=192.168.1.102
export CAMERA_PORT=554
#export CAMERA_SUBTYPE=0     # For high-resolution
export CAMERA_SUBTYPE=1     # For low-resolution 640x480 @20fps
export RTSP_URL="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}:${CAMERA_PORT}/cam/realmonitor?channel=1&subtype=${CAMERA_SUBTYPE}"
echo RTSP_URL: ${RTSP_URL}

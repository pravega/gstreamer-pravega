# Environment variables to the RTSP camera simulator.
export CAMERA_IP=127.0.0.1
export CAMERA_PORT=8554
export RTSP_URL="rtsp://${CAMERA_IP}:${CAMERA_PORT}/cam/realmonitor"
echo RTSP_URL: ${RTSP_URL}

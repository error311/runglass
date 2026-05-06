#!/usr/bin/env sh
set -eu

export COMPOSE_PROJECT_NAME="runglassstack"

mkdir -p config bin notes

cat > .env <<'EOF'
RUNGLASS_STACK_MODE=observed
RUNGLASS_EXPOSE_APP=1
EOF

cat > compose.override.yaml <<'EOF'
services:
  web:
    environment:
      RUNGLASS_STACK_MODE: ${RUNGLASS_STACK_MODE}
EOF

cat > config/app.toml <<'EOF'
name = "runglass-stack"
mode = "provisioned"
healthcheck = "enabled"
EOF

cat > bin/stack-healthcheck <<'EOF'
#!/usr/bin/env sh
curl -fsS http://127.0.0.1:18080 >/dev/null
echo "healthcheck ok"
EOF

chmod +x bin/stack-healthcheck

printf 'Preparing Docker stack receipt...\n'
docker compose up -d
printf 'Docker stack is running.\n'

attempt=0
until curl -fsS http://127.0.0.1:18080 >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 20 ]; then
    printf 'stack did not become healthy in time\n' >&2
    exit 1
  fi
  sleep 0.5
done

python3 - <<'PY'
import socket
import time

sock = socket.create_connection(("example.com", 80))
try:
    sock.sendall(b"HEAD / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
    time.sleep(0.5)
finally:
    sock.close()
PY

python3 -m http.server 8099 >/dev/null 2>&1 &
local_server=$!
sleep 0.5

python3 - <<'PY'
import socket
import time

sock = socket.create_connection(("127.0.0.1", 8099))
try:
    sock.sendall(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
    time.sleep(1.0)
finally:
    sock.close()
PY

kill "$local_server"
wait "$local_server" 2>/dev/null || true

./bin/stack-healthcheck

printf 'Receipt flow complete.\n' > notes/receipt-summary.txt
printf 'RunGlass stack flow finished.\n'

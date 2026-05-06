#!/usr/bin/env sh
set -eu

export COMPOSE_PROJECT_NAME="runglassstack"

docker compose down -v >/dev/null 2>&1 || true
rm -rf .env compose.override.yaml bin notes
mkdir -p config
cat > config/app.toml <<'EOF'
name = "runglass-stack"
mode = "baseline"
EOF

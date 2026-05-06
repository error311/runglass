#!/usr/bin/env sh
set -eu

mkdir -p config bin

cat > config/app.toml <<'EOF'
name = "runglass-demo"
mode = "local"
EOF

cat > bin/demo-tool <<'EOF'
#!/usr/bin/env sh
echo "demo tool ready"
EOF

chmod +x bin/demo-tool
printf 'installed\n'

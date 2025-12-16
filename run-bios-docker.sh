#!/bin/bash
# Run Kazeta+ BIOS in Docker with VNC display

set -e

echo "🎮 Kazeta+ BIOS Docker Runner (VNC Mode)"
echo "==========================================="

# Build the Docker image
echo ""
echo "📦 Building Docker image..."
docker build -f Dockerfile.dev -t kazeta-bios-dev .

# Create games directory if it doesn't exist
mkdir -p ~/kazeta-games

# Run the container with VNC
echo ""
echo "🚀 Starting BIOS with VNC server..."
echo ""
echo "📺 Connect with VNC viewer to: localhost:5900"
echo "   - macOS: Open Screen Sharing, connect to vnc://localhost:5900"
echo "   - Or install VNC Viewer: brew install --cask vnc-viewer"
echo ""

docker run --rm -it \
    -p 5900:5900 \
    -v ~/kazeta-games:/media:ro \
    -v "$(pwd):/workspace" \
    kazeta-bios-dev

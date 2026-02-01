#!/bin/bash
# Development script for Elm frontend
# Compiles Elm and starts a simple HTTP server

set -e

cd "$(dirname "$0")"

echo "Compiling Elm (debug mode)..."
elm make src/Main.elm --output=public/elm.js

echo ""
echo "Starting development server on http://localhost:3000"
echo "Press Ctrl+C to stop"
echo ""

# Use Python's built-in HTTP server
cd public
python3 -m http.server 3000

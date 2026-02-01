#!/bin/bash
# Build script for Elm frontend

set -e

echo "Building Elm application..."

# Compile Elm to JavaScript
elm make src/Main.elm --optimize --output=public/elm.js

# Minify (optional, requires uglifyjs)
if command -v uglifyjs &> /dev/null; then
    echo "Minifying..."
    uglifyjs public/elm.js --compress 'pure_funcs=[F2,F3,F4,F5,F6,F7,F8,F9,A2,A3,A4,A5,A6,A7,A8,A9],pure_getters,keep_fargs=false,unsafe_comps,unsafe' | uglifyjs --mangle --output public/elm.min.js
    mv public/elm.min.js public/elm.js
fi

echo "Build complete! Output in public/"
echo "  - public/index.html"
echo "  - public/elm.js"
echo "  - public/ports.js"
echo "  - public/styles.css"

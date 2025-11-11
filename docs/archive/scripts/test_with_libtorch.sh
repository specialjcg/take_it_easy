#!/bin/bash

# Set LibTorch environment variables
export LIBTORCH=/home/jcgouleau/libtorch-clean/libtorch
export LD_LIBRARY_PATH=/home/jcgouleau/libtorch-clean/libtorch/lib:$LD_LIBRARY_PATH

echo "🔧 Environment configured:"
echo "  LIBTORCH=$LIBTORCH"
echo "  LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
echo ""

# Run tests
echo "🧪 Running tests..."
cargo test "$@"
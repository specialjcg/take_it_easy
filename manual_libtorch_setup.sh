#!/bin/bash

# Fix TLS certificate issue by using manual LibTorch installation
set -e

echo "🔧 Fixing TLS certificate issue and configuring manual LibTorch..."

# Step 1: Check if manual LibTorch installation exists
LIBTORCH_PATH="$HOME/libtorch-clean/libtorch"

if [ ! -d "$LIBTORCH_PATH" ]; then
    echo "❌ Manual LibTorch installation not found at $LIBTORCH_PATH"
    echo "Let's download it manually with certificate verification disabled..."

    # Create directory
    mkdir -p "$HOME/libtorch-clean"
    cd "$HOME/libtorch-clean"

    # Download with certificate verification disabled
    echo "📥 Downloading LibTorch manually..."
    wget --no-check-certificate -O libtorch.zip "https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.6.0%2Bcpu.zip"

    # Extract
    echo "📦 Extracting LibTorch..."
    unzip -q libtorch.zip
    rm libtorch.zip

    echo "✅ Manual LibTorch download complete"
    cd - > /dev/null
fi

# Step 2: Verify the manual installation
echo "🔍 Verifying LibTorch installation..."

if [ ! -f "$LIBTORCH_PATH/include/torch/torch.h" ]; then
    echo "❌ torch/torch.h not found at $LIBTORCH_PATH/include/torch/torch.h"
    exit 1
fi

if [ ! -f "$LIBTORCH_PATH/lib/libtorch.so" ]; then
    echo "❌ libtorch.so not found at $LIBTORCH_PATH/lib/libtorch.so"
    exit 1
fi

echo "✅ LibTorch installation verified"

# Step 3: Configure environment for manual installation
echo "🔧 Configuring environment for manual LibTorch..."

# Clear automatic download variables
unset TORCH_CUDA_VERSION
unset LIBTORCH_USE_PYTORCH

# Set manual installation variables
export LIBTORCH="$LIBTORCH_PATH"
export LD_LIBRARY_PATH="$LIBTORCH_PATH/lib:$LD_LIBRARY_PATH"

echo "Environment variables set:"
echo "  LIBTORCH=$LIBTORCH"
echo "  LD_LIBRARY_PATH includes: $LIBTORCH_PATH/lib"

# Step 4: Update Cargo.toml to remove download feature
echo "📝 Updating Cargo.toml to use manual installation..."

if [ -f "Cargo.toml" ]; then
    # Create backup
    cp Cargo.toml "Cargo.toml.backup-$(date +%Y%m%d_%H%M%S)"

    # Remove download-libtorch feature if present
    if grep -q 'features.*download-libtorch' Cargo.toml; then
        echo "Removing download-libtorch feature from Cargo.toml..."
        sed -i.tmp 's/tch = { version = "\([^"]*\)", features = \["download-libtorch"\] }/tch = "\1"/' Cargo.toml
        rm -f Cargo.toml.tmp
        echo "✅ Removed download-libtorch feature"
    else
        echo "✅ No download-libtorch feature found"
    fi

    # Show current tch configuration
    echo "Current tch configuration in Cargo.toml:"
    grep -n "tch.*=" Cargo.toml || echo "No tch dependency found"

else
    echo "❌ Cargo.toml not found in current directory"
    echo "Please ensure you're in your Rust project directory"
    exit 1
fi

# Step 5: Clean and build
echo "🧹 Cleaning build artifacts..."
cargo clean
rm -rf target/

# Step 6: Try building with manual LibTorch
echo "🔨 Building with manual LibTorch installation..."
echo "This should now use your local LibTorch instead of downloading..."

if cargo build; then
    echo "✅ Build successful with manual LibTorch!"
    echo ""
    echo "🎉 Problem solved! Your project is now using manual LibTorch installation."
    echo ""
    echo "📍 LibTorch location: $LIBTORCH_PATH"
    echo "🔧 Configuration: Manual installation (no automatic download)"
    echo ""
    echo "To make this permanent, add these lines to your ~/.bashrc:"
    echo "export LIBTORCH=\"$LIBTORCH_PATH\""
    echo "export LD_LIBRARY_PATH=\"\$LIBTORCH/lib:\$LD_LIBRARY_PATH\""

else
    echo "❌ Build still failed. Let's diagnose the issue..."
    echo ""
    echo "Debugging information:"
    echo "- LibTorch path: $LIBTORCH_PATH"
    echo "- Headers exist: $(test -f "$LIBTORCH_PATH/include/torch/torch.h" && echo "YES" || echo "NO")"
    echo "- Library exists: $(test -f "$LIBTORCH_PATH/lib/libtorch.so" && echo "YES" || echo "NO")"
    echo "- LIBTORCH env var: $LIBTORCH"
    echo "- LD_LIBRARY_PATH: $LD_LIBRARY_PATH"
    echo ""
    echo "Try building with verbose output to see more details:"
    echo "cargo build -v"
fi

# Step 7: Alternative TLS fix for future downloads
echo ""
echo "🔒 For future reference, here are ways to fix TLS certificate issues:"
echo ""
echo "Option 1: Update CA certificates"
echo "  sudo apt update && sudo apt install ca-certificates"
echo ""
echo "Option 2: Configure git to use system certificates"
echo "  git config --global http.sslcainfo /etc/ssl/certs/ca-certificates.crt"
echo ""
echo "Option 3: Set curl CA bundle"
echo "  export CURL_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt"
echo ""
echo "The manual installation approach we just used bypasses these issues entirely."
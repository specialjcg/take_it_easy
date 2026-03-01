#!/bin/bash
# ═══════════════════════════════════════════════════════════════
#  Vast.ai GPU Setup — Batched MCTS Benchmark
# ═══════════════════════════════════════════════════════════════
#
# Utilisation :
#   1. Louer une instance Vast.ai (RTX 3090/4090, image cuda:12.4-devel)
#   2. Depuis ta machine locale :
#        scp -P <PORT> scripts/vastai_setup.sh root@<HOST>:/root/
#        scp -P <PORT> model_weights/graph_transformer_policy.safetensors root@<HOST>:/root/
#   3. SSH et lancer :
#        ssh -p <PORT> root@<HOST>
#        bash /root/vastai_setup.sh
#
set -euo pipefail

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Vast.ai GPU Setup — Take It Easy MCTS Benchmark        ║"
echo "╚══════════════════════════════════════════════════════════╝"

# ── 1. Installer les dépendances système ──
echo -e "\n[1/5] Installing system dependencies..."
apt-get update -qq && apt-get install -y -qq \
    curl build-essential pkg-config libssl-dev git wget unzip > /dev/null 2>&1
echo "  Done."

# ── 2. Installer Rust ──
if command -v rustc &> /dev/null; then
    echo -e "\n[2/5] Rust already installed: $(rustc --version)"
else
    echo -e "\n[2/5] Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y > /dev/null 2>&1
    source "$HOME/.cargo/env"
    echo "  Installed: $(rustc --version)"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# ── 3. Télécharger libtorch CUDA ──
LIBTORCH_DIR="/opt/libtorch"
if [ -d "$LIBTORCH_DIR" ]; then
    echo -e "\n[3/5] libtorch already present at $LIBTORCH_DIR"
else
    echo -e "\n[3/5] Downloading libtorch 2.5.1+cu124 (~3 GB)..."
    cd /opt
    wget -q --show-progress \
        "https://download.pytorch.org/libtorch/cu124/libtorch-cxx11-abi-shared-with-deps-2.5.1%2Bcu124.zip" \
        -O libtorch.zip
    echo "  Extracting..."
    unzip -q libtorch.zip && rm libtorch.zip
    echo "  Done."
fi
export LIBTORCH="$LIBTORCH_DIR"
export LD_LIBRARY_PATH="$LIBTORCH_DIR/lib:${LD_LIBRARY_PATH:-}"

# ── 4. Cloner le repo et préparer ──
echo -e "\n[4/5] Cloning repository..."
cd /root
if [ -d "take_it_easy" ]; then
    cd take_it_easy
    git fetch origin
    git checkout feature/cuda-gpu
    git pull origin feature/cuda-gpu || true
else
    git clone https://github.com/specialjcg/take_it_easy.git
    cd take_it_easy
    git checkout feature/cuda-gpu
fi

# Copier les poids si uploadés dans /root/
mkdir -p model_weights
if [ -f "/root/graph_transformer_policy.safetensors" ]; then
    cp /root/graph_transformer_policy.safetensors model_weights/
    echo "  Model weights copied."
elif [ -f "model_weights/graph_transformer_policy.safetensors" ]; then
    echo "  Model weights already in place."
else
    echo "  WARNING: model weights not found!"
    echo "  Upload with: scp -P <PORT> model_weights/graph_transformer_policy.safetensors root@<HOST>:/root/"
    exit 1
fi

# ── 5. Build ──
echo -e "\n[5/5] Building (release)... this takes ~5 min first time"
cargo build --release --bin benchmark_mcts_gpu 2>&1 | tail -3

# ── CUDA check ──
echo -e "\n═══════════════════════════════════════════════════"
echo "  Setup complete! Running CUDA check..."
echo "═══════════════════════════════════════════════════"
./target/release/benchmark_mcts_gpu --device cuda --num-games 1 --sim-counts "10" 2>&1 | head -12

echo -e "\n═══════════════════════════════════════════════════"
echo "  Ready! Run the full benchmark with:"
echo ""
echo "  # Quick test (5 min)"
echo "  ./target/release/benchmark_mcts_gpu \\"
echo "    --device cuda --num-games 50 --sim-counts \"50,100,500\" --batch-size 64"
echo ""
echo "  # Full benchmark (30-60 min)"
echo "  ./target/release/benchmark_mcts_gpu \\"
echo "    --device cuda --num-games 100 --sim-counts \"50,100,500,1000,5000\" --batch-size 128"
echo "═══════════════════════════════════════════════════"

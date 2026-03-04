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
echo "║  Vast.ai GPU Setup — Take It Easy Training & Benchmark  ║"
echo "╚══════════════════════════════════════════════════════════╝"

# ── 1. Installer les dépendances système ──
echo -e "\n[1/5] Installing system dependencies..."
apt-get update -qq && apt-get install -y -qq \
    curl build-essential pkg-config libssl-dev git wget unzip protobuf-compiler > /dev/null 2>&1
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
    git checkout master
    git pull origin master || true
else
    git clone https://github.com/specialjcg/take_it_easy.git
    cd take_it_easy
fi

# Override .cargo/config.toml to point to CUDA libtorch (repo has local CPU paths)
mkdir -p .cargo
cat > .cargo/config.toml <<'TOML'
[env]
LIBTORCH = { value = "/opt/libtorch", force = true }
LD_LIBRARY_PATH = { value = "/opt/libtorch/lib", force = true }

[build]

[target.'cfg(test)']
rustflags = ["-C", "link-arg=-Wl,-rpath=/opt/libtorch/lib"]
TOML
echo "  .cargo/config.toml overwritten for CUDA libtorch."

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

# ── 5. Verify CUDA libs & Build ──
echo -e "\n[5/5] Verifying libtorch CUDA and building..."
if [ -f "$LIBTORCH_DIR/lib/libtorch_cuda.so" ]; then
    echo "  libtorch_cuda.so found — CUDA linking will be enabled."
else
    echo "  ERROR: libtorch_cuda.so NOT found in $LIBTORCH_DIR/lib/"
    echo "  torch-sys will only link CPU. Check your libtorch installation."
    ls "$LIBTORCH_DIR/lib/"libtorch*.so 2>/dev/null || echo "  No libtorch*.so files found!"
    exit 1
fi
# Clean torch-sys cache to force re-detection of CUDA libs
cargo clean -p torch-sys 2>/dev/null || true
cargo build --release --bin benchmark_mcts_gpu --bin train_value_net --bin train_graph_transformer 2>&1 | tail -5

# ── CUDA check ──
echo -e "\n═══════════════════════════════════════════════════"
echo "  Setup complete! Running CUDA check..."
echo "═══════════════════════════════════════════════════"
./target/release/benchmark_mcts_gpu --device cuda --num-games 1 --sim-counts "10" 2>&1 | head -12

echo -e "\n═══════════════════════════════════════════════════"
echo "  Ready! Commands:"
echo ""
echo "  # 1. Train GT Big (dim=256, 4 layers, 8 heads) — 100k self-play"
echo "  ./target/release/train_graph_transformer \\"
echo "    --device cuda \\"
echo "    --gen-games 100000 \\"
echo "    --policy-path model_weights/graph_transformer_policy.safetensors \\"
echo "    --embed-dim 256 --num-layers 4 --heads 8 \\"
echo "    --dropout 0.2 --batch-size 128 --lr 0.0003 --weight-decay 0.0003 \\"
echo "    --epochs 100 --weight-power 3.0 \\"
echo "    --save-path model_weights/gt_big"
echo ""
echo "  # 2. Train value network (GPU, ~10 min for 10k games)"
echo "  ./target/release/train_value_net \\"
echo "    --device cuda --num-games 10000 --epochs 80 --eval-games 200"
echo ""
echo "  # 3. Benchmark expectimax vs GT Direct"
echo "  ./target/release/benchmark_mcts_gpu \\"
echo "    --device cuda --num-games 200 \\"
echo "    --value-model-path model_weights/value_net.safetensors \\"
echo "    --sim-counts \"50,100\""
echo "═══════════════════════════════════════════════════"

#!/bin/bash
# ═══════════════════════════════════════════════════════════════
#  Vast.ai GPU Setup — Take It Easy Training
# ═══════════════════════════════════════════════════════════════
#
# Script 100% autonome. Sur une nouvelle instance Vast.ai :
#
#   1. Louer une instance (RTX 3090/4090, image pytorch/pytorch:2.5.1-cuda12.4-cudnn9-devel)
#   2. Depuis ta machine locale :
#        scp -P <PORT> scripts/vastai_setup.sh root@<HOST>:/root/
#        scp -P <PORT> model_weights/graph_transformer_policy.safetensors root@<HOST>:/root/
#   3. SSH et lancer :
#        ssh -p <PORT> root@<HOST>
#        bash /root/vastai_setup.sh
#
# Le script crée aussi /root/run_training.sh pour relancer facilement.
#
set -euo pipefail

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Vast.ai GPU Setup — Take It Easy Training              ║"
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

# ── 3. Détecter libtorch (PyTorch Python ou /opt/libtorch) ──
echo -e "\n[3/5] Detecting libtorch..."
USE_PYTORCH=0
if python3 -c "import torch; assert torch.cuda.is_available()" 2>/dev/null; then
    TORCH_LIB=$(python3 -c "import torch; print(torch.__file__.replace('__init__.py','lib'))")
    echo "  Found PyTorch Python with CUDA: $TORCH_LIB"
    USE_PYTORCH=1
elif [ -f "/opt/libtorch/lib/libtorch_cuda.so" ]; then
    echo "  Found /opt/libtorch with CUDA"
else
    echo "  No CUDA libtorch found. Downloading libtorch 2.5.1+cu124 (~3 GB)..."
    cd /opt
    wget -q --show-progress \
        "https://download.pytorch.org/libtorch/cu124/libtorch-cxx11-abi-shared-with-deps-2.5.1%2Bcu124.zip" \
        -O libtorch.zip
    echo "  Extracting..."
    unzip -q libtorch.zip && rm libtorch.zip
    echo "  Done."
fi

# ── 4. Cloner le repo et configurer ──
echo -e "\n[4/5] Setting up repository..."
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

# Configure cargo pour le bon libtorch
mkdir -p .cargo
if [ "$USE_PYTORCH" -eq 1 ]; then
    cat > .cargo/config.toml <<'TOML'
[env]
LIBTORCH_USE_PYTORCH = { value = "1", force = true }

[build]
TOML
    echo "  .cargo/config.toml → LIBTORCH_USE_PYTORCH=1"
else
    cat > .cargo/config.toml <<'TOML'
[env]
LIBTORCH = { value = "/opt/libtorch", force = true }
LD_LIBRARY_PATH = { value = "/opt/libtorch/lib", force = true }

[build]
TOML
    echo "  .cargo/config.toml → /opt/libtorch"
fi

# Copier les poids
mkdir -p model_weights
if [ -f "/root/graph_transformer_policy.safetensors" ]; then
    cp /root/graph_transformer_policy.safetensors model_weights/
    echo "  Model weights copied."
elif [ -f "model_weights/graph_transformer_policy.safetensors" ]; then
    echo "  Model weights already in place."
else
    echo "  WARNING: model weights not found!"
    echo "  Upload: scp -P <PORT> model_weights/graph_transformer_policy.safetensors root@<HOST>:/root/"
    exit 1
fi

# ── 5. Build ──
echo -e "\n[5/5] Building binaries..."
# Nuke torch-sys cache pour forcer re-détection
rm -rf target/release/build/torch-sys-* target/release/.fingerprint/torch-sys-* \
       target/release/build/tch-* target/release/.fingerprint/tch-* \
       target/release/.fingerprint/take_it_easy-* 2>/dev/null || true

cargo build --release \
    --bin train_graph_transformer \
    --bin train_value_net \
    --bin train_sheaf \
    --bin train_hypergraph \
    --bin benchmark_mcts_gpu 2>&1 | tail -5

# ── Construire le LD_LIBRARY_PATH et LD_PRELOAD pour CUDA ──
if [ "$USE_PYTORCH" -eq 1 ]; then
    NVIDIA_LIBS=$(python3 -c "
import os, glob
base = '/usr/local/lib/python3.12/dist-packages'
paths = glob.glob(os.path.join(base, 'nvidia', '*', 'lib'))
paths.append(os.path.join(base, 'torch', 'lib'))
print(':'.join(paths))
" 2>/dev/null || echo "")
    CUDA_LD="$NVIDIA_LIBS:/usr/local/cuda/lib64:/usr/lib/x86_64-linux-gnu"
    CUDA_PRELOAD="$TORCH_LIB/libtorch_cuda.so"
else
    CUDA_LD="/opt/libtorch/lib:/usr/local/cuda/lib64:/usr/lib/x86_64-linux-gnu"
    CUDA_PRELOAD=""
fi

# ── CUDA check ──
echo -e "\n═══════════════════════════════════════════════════"
echo "  Running CUDA check..."
echo "═══════════════════════════════════════════════════"
LD_LIBRARY_PATH="$CUDA_LD" LD_PRELOAD="${CUDA_PRELOAD:-}" \
    ./target/release/train_graph_transformer --device cuda --gen-games 0 --epochs 0 2>&1 | head -5

# ── Créer le script de lancement ──
cat > /root/run_training.sh << LAUNCHER
#!/bin/bash
# Lance le training GT Big (dim=256, 4 layers, 8 heads)
cd /root/take_it_easy
export LD_LIBRARY_PATH="$CUDA_LD"
export LD_PRELOAD="${CUDA_PRELOAD:-}"

./target/release/train_graph_transformer \\
  --device cuda \\
  --gen-games 100000 \\
  --policy-path model_weights/graph_transformer_policy.safetensors \\
  --gen-embed-dim 128 --gen-num-layers 2 --gen-heads 4 \\
  --embed-dim 256 --num-layers 4 --heads 8 \\
  --dropout 0.2 --batch-size 128 --lr 0.0003 --weight-decay 0.0003 \\
  --epochs 100 --weight-power 3.0 \\
  --save-path model_weights/gt_big \\
  2>&1 | tee training_gt_big.log
LAUNCHER
chmod +x /root/run_training.sh

cat > /root/run_training_bg.sh << LAUNCHER
#!/bin/bash
# Lance le training en background (survit à la déconnexion SSH)
cd /root/take_it_easy
export LD_LIBRARY_PATH="$CUDA_LD"
export LD_PRELOAD="${CUDA_PRELOAD:-}"

nohup ./target/release/train_graph_transformer \\
  --device cuda \\
  --gen-games 100000 \\
  --policy-path model_weights/graph_transformer_policy.safetensors \\
  --gen-embed-dim 128 --gen-num-layers 2 --gen-heads 4 \\
  --embed-dim 256 --num-layers 4 --heads 8 \\
  --dropout 0.2 --batch-size 128 --lr 0.0003 --weight-decay 0.0003 \\
  --epochs 100 --weight-power 3.0 \\
  --save-path model_weights/gt_big \\
  > training_gt_big.log 2>&1 &

echo "PID=\$!"
echo "Suivi: tail -f /root/take_it_easy/training_gt_big.log"
LAUNCHER
chmod +x /root/run_training_bg.sh

cat > /root/run_sheaf.sh << LAUNCHER
#!/bin/bash
# Sheaf Neural Network Training (GPU)
cd /root/take_it_easy
export LD_LIBRARY_PATH="$CUDA_LD"
export LD_PRELOAD="${CUDA_PRELOAD:-}"

./target/release/train_sheaf \\
  --device cuda \\
  --gen-games 100000 \\
  --policy-path model_weights/graph_transformer_policy.safetensors \\
  --gen-embed-dim 128 --gen-num-layers 2 --gen-heads 4 \\
  --embed-dim 128 --stalk-dim 64 --num-layers 3 \\
  --dropout 0.1 --batch-size 128 --lr 0.0005 --weight-decay 0.0001 \\
  --epochs 80 --weight-power 3.0 --patience 3 \\
  --save-path model_weights/sheaf \\
  2>&1 | tee training_sheaf.log
LAUNCHER
chmod +x /root/run_sheaf.sh

cat > /root/run_sheaf_bg.sh << LAUNCHER
#!/bin/bash
cd /root/take_it_easy
export LD_LIBRARY_PATH="$CUDA_LD"
export LD_PRELOAD="${CUDA_PRELOAD:-}"

nohup ./target/release/train_sheaf \\
  --device cuda \\
  --gen-games 100000 \\
  --policy-path model_weights/graph_transformer_policy.safetensors \\
  --gen-embed-dim 128 --gen-num-layers 2 --gen-heads 4 \\
  --embed-dim 128 --stalk-dim 64 --num-layers 3 \\
  --dropout 0.1 --batch-size 128 --lr 0.0005 --weight-decay 0.0001 \\
  --epochs 80 --weight-power 3.0 --patience 3 \\
  --save-path model_weights/sheaf \\
  > training_sheaf.log 2>&1 &

echo "PID=\$!"
echo "Suivi: tail -f /root/take_it_easy/training_sheaf.log"
LAUNCHER
chmod +x /root/run_sheaf_bg.sh

echo -e "\n═══════════════════════════════════════════════════"
echo "  Setup complete!"
echo ""
echo "  Training scripts:"
echo "    bash /root/run_training.sh       # GT Big (foreground)"
echo "    bash /root/run_sheaf.sh          # Sheaf Network (foreground)"
echo "    bash /root/run_sheaf_bg.sh       # Sheaf Network (background)"
echo ""
echo "  Suivi :"
echo "    tail -f /root/take_it_easy/training_sheaf.log"
echo ""
echo "  Recuperer les modeles :"
echo "    scp -P <PORT> root@<HOST>:/root/take_it_easy/model_weights/sheaf_policy.safetensors ."
echo "    scp -P <PORT> root@<HOST>:/root/take_it_easy/model_weights/gt_big_policy.safetensors ."
echo "═══════════════════════════════════════════════════"

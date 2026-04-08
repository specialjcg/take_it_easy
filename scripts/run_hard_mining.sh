#!/bin/bash
# Hard Example Mining Pipeline — Run on GPU machine
# Usage: bash scripts/run_hard_mining.sh
set -e

export PATH=$HOME/bin:$HOME/.cargo/bin:$PATH
export PROTOC=$HOME/bin/protoc
export LIBTORCH=$HOME/libtorch
export LD_LIBRARY_PATH=$LIBTORCH/lib:$LD_LIBRARY_PATH
export CUDA_VISIBLE_DEVICES=1

cd ~/take_it_easy

echo "=== Step 0: Build ==="
cargo build --release --bin hard_example_mining --bin distill_expectimax 2>&1 | tail -3

echo ""
echo "=== Step 1+2+3: Hard Example Mining (2-ply expert, 50 worst) ==="
./target/release/hard_example_mining \
  --device cpu \
  --policy-path model_weights/distilled_v2_policy.safetensors \
  --value-path model_weights/value_net_v2_notanh.safetensors \
  --num-games 5000 \
  --threshold 100 \
  --depth 2 \
  --min-turn 6 \
  --output-tsv data/hard_examples_2ply.tsv \
  --baseline-tsv data/hard_examples_v2_baseline.tsv

echo ""
echo "=== Step 4: Merge training data ==="
# Keep header from original, append hard examples (skip header)
cp data/expectimax_2ply_10k.tsv data/mixed_v3_training.tsv
tail -n +2 data/hard_examples_2ply.tsv >> data/mixed_v3_training.tsv
echo "Mixed data: $(wc -l < data/mixed_v3_training.tsv) lines"

echo ""
echo "=== Step 5: Fine-tune V2 → V3 ==="
# Preserve V2
cp model_weights/distilled_v2_policy.safetensors model_weights/distilled_v3_risk.safetensors

./target/release/distill_expectimax \
  --device cpu \
  --policy-path model_weights/distilled_v2_policy.safetensors \
  --value-path model_weights/value_net_v2_notanh.safetensors \
  --save-path model_weights/distilled_v3_risk.safetensors \
  --load-tsv data/mixed_v3_training.tsv \
  --weight-mode standard \
  --num-games 0 \
  --epochs 20 \
  --lr 0.0002 \
  --eval-games 500 \
  --depth 1 \
  --min-turn 8

echo ""
echo "=== Done! ==="
echo "V3 model: model_weights/distilled_v3_risk.safetensors"
echo "Compare with V2: check the eval output above"

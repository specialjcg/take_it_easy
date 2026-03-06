#!/bin/bash
set -euo pipefail
cd /app

cuda_check() {
    echo "CUDA check..."
    python3 -c "import torch; print(f'CUDA: {torch.cuda.is_available()}, Device: {torch.cuda.get_device_name(0) if torch.cuda.is_available() else \"N/A\"}')" 2>/dev/null || \
    ./target/release/train_graph_transformer --device cuda --gen-games 0 --epochs 0 2>&1 | head -3
    echo ""
}

case "${1:-help}" in
    help)
        cat <<'EOF'
================================================
  Take It Easy — GPU Training Container
================================================

  docker run --gpus all IMAGE cuda-check
  docker run --gpus all -v data:/app/model_weights IMAGE train-gt [ARGS...]
  docker run --gpus all -v data:/app/model_weights IMAGE train-hyper [ARGS...]
  docker run --gpus all -v data:/app/model_weights IMAGE train-value [ARGS...]
  docker run --gpus all -v data:/app/model_weights IMAGE distill [ARGS...]
  docker run --gpus all -v data:/app/model_weights IMAGE benchmark [ARGS...]
  docker run --gpus all -v data:/app/model_weights IMAGE bash

Model weights in /app/model_weights/
================================================
EOF
        ;;
    cuda-check)
        cuda_check
        nvidia-smi 2>/dev/null || echo "nvidia-smi not available"
        ;;
    train-gt)
        shift; cuda_check
        exec ./target/release/train_graph_transformer --device cuda "$@"
        ;;
    train-hyper)
        shift; cuda_check
        exec ./target/release/train_hypergraph --device cuda "$@"
        ;;
    train-value)
        shift; cuda_check
        exec ./target/release/train_value_net --device cuda "$@"
        ;;
    distill)
        shift; cuda_check
        exec ./target/release/distill_expectimax --device cuda "$@"
        ;;
    benchmark)
        shift
        exec ./target/release/benchmark_strategies "$@"
        ;;
    bash|sh)
        exec /bin/bash
        ;;
    *)
        exec "$@"
        ;;
esac

#!/bin/bash
# Example smoke template: echo params back as structured JSON.
# Parameters (set as env vars by the engine):
#   EXPERIMENT_LR       — learning rate
#   EXPERIMENT_BATCH_SIZE — batch size
#   EXPERIMENT_MODEL    — model name (if provided)
#
# Any parameter key in the input becomes EXPERIMENT_<KEY>=<VALUE>.

LR="${EXPERIMENT_LR:-0.01}"
BS="${EXPERIMENT_BATCH_SIZE:-32}"
MODEL="${EXPERIMENT_MODEL:-default}"

cat <<EOF
{"lr": $LR, "batch_size": $BS, "model": "$MODEL", "status": "ok"}
EOF

#!/bin/bash
# Example: simulate a quick training probe experiment.
# Parameters:
#   EXPERIMENT_LR          — learning rate (float)
#   EXPERIMENT_BATCH_SIZE  — batch size (int)
#   EXPERIMENT_EPOCHS      — number of epochs (int, default 1)
#
# Replace this file's body with your actual experiment logic.
# The engine passes all EXPERIMENT_* env vars. Output JSON to stdout.

LR="${EXPERIMENT_LR:-0.01}"
BS="${EXPERIMENT_BATCH_SIZE:-32}"
EPOCHS="${EXPERIMENT_EPOCHS:-1}"

# Simulate a quick probe (replace with actual train/eval command)
sleep 0.3

# Fake result — replace with real metrics from your experiment
echo "{\"lr\": $LR, \"batch_size\": $BS, \"epochs\": $EPOCHS, \"accuracy\": 0.85, \"loss\": 0.35, \"samples_per_sec\": 1234}"

#!/bin/bash

# Script to filter log file for specific prefixes
# Usage: ./filter_logs.sh <input_logfile> [output_logfile]

if [ $# -lt 1 ]; then
    echo "Usage: $0 <input_logfile> [output_logfile]"
    echo "  If output_logfile is not specified, results will be printed to stdout"
    exit 1
fi

INPUT_FILE="$1"
OUTPUT_FILE="$2"

if [ ! -f "$INPUT_FILE" ]; then
    echo "Error: Input file '$INPUT_FILE' does not exist"
    exit 1
fi

# Filter lines that start with QUEUE_MANAGER_SAMPLES or CURRENT_OCCUPANCY
if [ -n "$OUTPUT_FILE" ]; then
    grep "^QUEUE_MANAGER_SAMPLES\|^CURRENT_OCCUPANCY" "$INPUT_FILE" > "$OUTPUT_FILE"
    echo "Filtered log saved to: $OUTPUT_FILE"
else
    grep "^QUEUE_MANAGER_SAMPLES\|^CURRENT_OCCUPANCY" "$INPUT_FILE"
fi
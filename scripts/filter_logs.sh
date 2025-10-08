#!/bin/bash

# Script to filter log file for lines containing specific keywords
# Usage: ./filter_logs.sh <input_logfile> --f KEYWORD1,KEYWORD2,... [output_logfile]

show_usage() {
    echo "Usage: $0 <input_logfile> --f KEYWORD1,KEYWORD2,... [output_logfile]"
    echo ""
    echo "  input_logfile    : Path to the log file to filter"
    echo "  --f              : Filter flag followed by comma-separated keywords"
    echo "  output_logfile   : Optional output file (prints to stdout if omitted)"
    echo ""
    echo "Example: $0 app.log --f QUEUE_MANAGER,CURRENT_OCCUPANCY filtered.log"
    exit 1
}

if [ $# -lt 3 ]; then
    show_usage
fi

INPUT_FILE="$1"
FILTER_FLAG="$2"
KEYWORDS="$3"
OUTPUT_FILE="$4"

if [ ! -f "$INPUT_FILE" ]; then
    echo "Error: Input file '$INPUT_FILE' does not exist"
    exit 1
fi

if [ "$FILTER_FLAG" != "--f" ]; then
    echo "Error: Second argument must be --f"
    show_usage
fi

# Convert comma-separated keywords to grep pattern
# e.g., "QUEUE_MANAGER,CURRENT_OCCUPANCY" -> "QUEUE_MANAGER\|CURRENT_OCCUPANCY"
GREP_PATTERN=$(echo "$KEYWORDS" | sed 's/,/\\|/g')

# Filter lines that contain any of the keywords
if [ -n "$OUTPUT_FILE" ]; then
    grep "$GREP_PATTERN" "$INPUT_FILE" > "$OUTPUT_FILE"
    echo "Filtered log saved to: $OUTPUT_FILE"
    echo "Matched $(wc -l < "$OUTPUT_FILE") lines"
else
    grep "$GREP_PATTERN" "$INPUT_FILE"
fi
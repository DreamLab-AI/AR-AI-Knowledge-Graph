#!/bin/bash

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Log file path
LOG_FILE="scripts/logs/rust.log"

# Function to display usage
usage() {
    echo -e "${YELLOW}Usage:${NC}"
    echo -e "  ./scripts/check-rust-logs.sh [options]"
    echo -e "\n${YELLOW}Options:${NC}"
    echo -e "  -f, --follow    Follow the log file (like tail -f)"
    echo -e "  -n, --lines N   Show last N lines (default: 20)"
    echo -e "  -r, --rotated N Show rotated log file number N (1-3)"
    echo -e "  -h, --help      Show this help message"
    echo -e "\n${YELLOW}Examples:${NC}"
    echo -e "  ./scripts/check-rust-logs.sh            # Show last 20 lines of current log"
    echo -e "  ./scripts/check-rust-logs.sh -f         # Follow the current log file"
    echo -e "  ./scripts/check-rust-logs.sh -n 50      # Show last 50 lines of current log"
    echo -e "  ./scripts/check-rust-logs.sh -r 1       # Show rotated log file 1 (most recent)"
}

# Default values
FOLLOW=false
LINES=20
ROTATED_LOG=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--follow)
            FOLLOW=true
            shift
            ;;
        -n|--lines)
            LINES="$2"
            shift 2
            ;;
        -r|--rotated)
            ROTATED_LOG="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${YELLOW}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Determine which log file to use
if [ -n "$ROTATED_LOG" ]; then
    ROTATED_FILE="${LOG_FILE}.${ROTATED_LOG}"
    if [ ! -f "$ROTATED_FILE" ]; then
        echo -e "${YELLOW}Rotated log file not found: $ROTATED_FILE${NC}"
        echo -e "Available log files:"
        ls -l "${LOG_FILE}"* 2>/dev/null || echo "No log files found."
        exit 1
    fi
    ACTIVE_LOG="$ROTATED_FILE"
    echo -e "${GREEN}Viewing rotated log file: $ROTATED_FILE${NC}"
    FOLLOW=false  # Can't follow rotated logs
else
    ACTIVE_LOG="$LOG_FILE"
    # Check if log file exists
    if [ ! -f "$ACTIVE_LOG" ]; then
        echo -e "${YELLOW}Log file not found: $ACTIVE_LOG${NC}"
        echo -e "Make sure the development server is running."
        exit 1
    fi
fi

# Display log file
echo -e "${GREEN}Rust Server Logs:${NC}"
if [ "$FOLLOW" = true ]; then
    echo -e "${YELLOW}Following log file. Press Ctrl+C to exit.${NC}"
    tail -f "$ACTIVE_LOG"
else
    tail -n "$LINES" "$ACTIVE_LOG"
fi

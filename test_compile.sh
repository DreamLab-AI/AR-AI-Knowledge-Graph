#!/bin/bash
cd /workspace/ext
# Test if cargo can check the code
cargo check 2>&1 | tail -100
#!/bin/bash

echo "Testing CUDA compilation..."

# Test with different architectures
for ARCH in 70 75 80 86 89; do
    echo "Testing sm_${ARCH}..."
    nvcc -arch=sm_${ARCH} -ptx test_cuda_simple.cu -o test_${ARCH}.ptx 2>&1
    if [ $? -eq 0 ]; then
        echo "✓ sm_${ARCH} succeeded"
        rm -f test_${ARCH}.ptx
    else
        echo "✗ sm_${ARCH} failed"
    fi
done

# Also test the actual compute_forces.cu
echo -e "\nTesting compute_forces.cu compilation..."
CUDA_ARCH=${CUDA_ARCH:-86}
nvcc -arch=sm_${CUDA_ARCH} -O3 --use_fast_math -ptx -rdc=true --compiler-options -fPIC src/utils/compute_forces.cu -o src/utils/compute_forces_test.ptx 2>&1

if [ $? -eq 0 ]; then
    echo "✓ compute_forces.cu compiled successfully"
    rm -f src/utils/compute_forces_test.ptx
else
    echo "✗ compute_forces.cu compilation failed"
fi
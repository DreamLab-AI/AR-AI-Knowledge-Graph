// Simple CUDA test to verify compilation
#include <cuda_runtime.h>

extern "C" {
    __global__ void simple_kernel(float* data, int n) {
        int idx = blockIdx.x * blockDim.x + threadIdx.x;
        if (idx < n) {
            data[idx] = data[idx] * 2.0f;
        }
    }
}
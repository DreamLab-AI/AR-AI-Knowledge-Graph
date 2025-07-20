// Simple test to verify GPU compilation
fn main() {
    println!("Testing GPU build...");
    
    #[cfg(feature = "gpu")]
    {
        println!("GPU feature is enabled");
        // Just test that we can import cudarc
        use cudarc::driver::CudaDevice;
        println!("cudarc imported successfully");
    }
    
    #[cfg(not(feature = "gpu"))]
    {
        println!("GPU feature is NOT enabled");
    }
}
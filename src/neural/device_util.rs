//! Device parsing utilities for CPU/CUDA selection.

use tch::Device;

/// Parse a device string like "cpu", "cuda", "cuda:0", "cuda:1".
pub fn parse_device(s: &str) -> Result<Device, String> {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "cpu" => Ok(Device::Cpu),
        "cuda" => Ok(Device::Cuda(0)),
        _ if s.starts_with("cuda:") => {
            let idx = s[5..]
                .parse::<usize>()
                .map_err(|e| format!("invalid CUDA device index: {}", e))?;
            Ok(Device::Cuda(idx))
        }
        _ => Err(format!("unknown device '{}' (expected cpu, cuda, cuda:N)", s)),
    }
}

/// Print CUDA diagnostic information.
pub fn check_cuda() {
    println!("CUDA available:  {}", tch::Cuda::is_available());
    println!("cuDNN available: {}", tch::Cuda::cudnn_is_available());
    println!("CUDA devices:    {}", tch::Cuda::device_count());
}

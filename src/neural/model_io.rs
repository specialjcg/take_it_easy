//! Model I/O utilities using safetensors format
//!
//! This module provides portable model serialization that works across
//! different libtorch versions by using the safetensors format instead
//! of PyTorch's native serialization.

use safetensors::tensor::{Dtype, SafeTensors, TensorView};
use safetensors::serialize_to_file;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tch::{nn, Kind, Tensor};

/// Save a VarStore to a safetensors file
pub fn save_varstore(vs: &nn::VarStore, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let mut tensors: HashMap<String, Vec<u8>> = HashMap::new();
    let mut metadata: HashMap<String, TensorMetadata> = HashMap::new();

    // Iterate over all named tensors in the VarStore
    for (name, tensor) in vs.variables() {
        let size: Vec<usize> = tensor.size().iter().map(|&x| x as usize).collect();
        let kind = tensor.kind();

        // Convert tensor to bytes
        let (data, dtype) = tensor_to_bytes(&tensor, kind)?;

        metadata.insert(name.clone(), TensorMetadata {
            shape: size.clone(),
            dtype: dtype_to_string(dtype),
        });

        tensors.insert(name, data);
    }

    // Create tensor views for safetensors
    let tensor_views: HashMap<String, TensorView<'_>> = tensors
        .iter()
        .map(|(name, data)| {
            let meta = &metadata[name];
            let dtype = string_to_dtype(&meta.dtype);
            (name.clone(), TensorView::new(dtype, meta.shape.clone(), data).unwrap())
        })
        .collect();

    // Serialize to file
    serialize_to_file(tensor_views, &None, path.as_ref())?;

    Ok(())
}

/// Load a VarStore from a safetensors file
pub fn load_varstore(vs: &mut nn::VarStore, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    // Read file
    let mut file = File::open(path.as_ref())?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Parse safetensors
    let tensors = SafeTensors::deserialize(&buffer)?;

    // Load each tensor into the VarStore
    for (name, mut var) in vs.variables() {
        if let Ok(tensor_view) = tensors.tensor(&name) {
            let loaded_tensor = tensor_view_to_tensor(&tensor_view)?;

            // Copy the loaded tensor to the variable
            tch::no_grad(|| {
                var.copy_(&loaded_tensor);
            });
        } else {
            eprintln!("Warning: tensor '{}' not found in safetensors file", name);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct TensorMetadata {
    shape: Vec<usize>,
    dtype: String,
}

fn tensor_to_bytes(tensor: &Tensor, kind: Kind) -> Result<(Vec<u8>, Dtype), Box<dyn std::error::Error>> {
    // Flatten the tensor for conversion, then get contiguous data on CPU
    let tensor = tensor.to_device(tch::Device::Cpu).flatten(0, -1).contiguous();

    match kind {
        Kind::Float => {
            let data: Vec<f32> = Vec::<f32>::try_from(&tensor)?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::F32))
        }
        Kind::Double => {
            let data: Vec<f64> = Vec::<f64>::try_from(&tensor)?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::F64))
        }
        Kind::Int => {
            let data: Vec<i32> = Vec::<i32>::try_from(&tensor)?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::I32))
        }
        Kind::Int64 => {
            let data: Vec<i64> = Vec::<i64>::try_from(&tensor)?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::I64))
        }
        Kind::Half => {
            // Convert to f32 for storage, will be converted back on load
            let data: Vec<f32> = Vec::<f32>::try_from(&tensor.to_kind(Kind::Float))?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::F32))
        }
        Kind::BFloat16 => {
            // Convert to f32 for storage
            let data: Vec<f32> = Vec::<f32>::try_from(&tensor.to_kind(Kind::Float))?;
            let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
            Ok((bytes, Dtype::F32))
        }
        _ => Err(format!("Unsupported tensor kind: {:?}", kind).into()),
    }
}

fn tensor_view_to_tensor(view: &TensorView) -> Result<Tensor, Box<dyn std::error::Error>> {
    let shape: Vec<i64> = view.shape().iter().map(|&x| x as i64).collect();
    let data = view.data();

    match view.dtype() {
        Dtype::F32 => {
            let floats: Vec<f32> = data
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok(Tensor::from_slice(&floats).reshape(&shape))
        }
        Dtype::F64 => {
            let doubles: Vec<f64> = data
                .chunks_exact(8)
                .map(|chunk| {
                    f64::from_le_bytes([
                        chunk[0], chunk[1], chunk[2], chunk[3],
                        chunk[4], chunk[5], chunk[6], chunk[7],
                    ])
                })
                .collect();
            Ok(Tensor::from_slice(&doubles).reshape(&shape))
        }
        Dtype::I32 => {
            let ints: Vec<i32> = data
                .chunks_exact(4)
                .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok(Tensor::from_slice(&ints).reshape(&shape))
        }
        Dtype::I64 => {
            let longs: Vec<i64> = data
                .chunks_exact(8)
                .map(|chunk| {
                    i64::from_le_bytes([
                        chunk[0], chunk[1], chunk[2], chunk[3],
                        chunk[4], chunk[5], chunk[6], chunk[7],
                    ])
                })
                .collect();
            Ok(Tensor::from_slice(&longs).reshape(&shape))
        }
        _ => Err(format!("Unsupported dtype: {:?}", view.dtype()).into()),
    }
}

fn dtype_to_string(dtype: Dtype) -> String {
    match dtype {
        Dtype::F32 => "F32".to_string(),
        Dtype::F64 => "F64".to_string(),
        Dtype::I32 => "I32".to_string(),
        Dtype::I64 => "I64".to_string(),
        _ => "F32".to_string(),
    }
}

fn string_to_dtype(s: &str) -> Dtype {
    match s {
        "F32" => Dtype::F32,
        "F64" => Dtype::F64,
        "I32" => Dtype::I32,
        "I64" => Dtype::I64,
        _ => Dtype::F32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_roundtrip() {
        let vs1 = nn::VarStore::new(tch::Device::Cpu);
        let _layer = nn::linear(&vs1.root() / "test", 10, 5, Default::default());

        let path = "/tmp/test_model.safetensors";
        save_varstore(&vs1, path).unwrap();

        let mut vs2 = nn::VarStore::new(tch::Device::Cpu);
        let _layer2 = nn::linear(&vs2.root() / "test", 10, 5, Default::default());
        load_varstore(&mut vs2, path).unwrap();

        // Check that tensors match
        for (name, t1) in vs1.variables() {
            let t2 = vs2.variables().into_iter().find(|(n, _)| n == &name).unwrap().1;
            assert!(t1.allclose(&t2, 1e-5, 1e-5, false));
        }

        std::fs::remove_file(path).ok();
    }
}

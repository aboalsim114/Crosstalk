//! Loads `pytorch_model.bin` in both PyTorch formats.
//!
//! - **Zip** (PyTorch ≥ 1.6): read by candle.
//! - **Legacy** (`torch.save` before 1.6, `_use_new_zipfile_serialization=False`):
//!   the format of every Helsinki-NLP Opus-MT checkpoint, uploaded in 2020.
//!   It is a sequence of pickles followed by the raw storages:
//!   1. magic number, 2. protocol version (1001), 3. system info,
//!   4. the state dict (tensors refer to storages by persistent id
//!      `('storage', FloatStorage, key, location, numel, view_metadata)`),
//!   5. the sorted list of storage keys,
//!   6. for each key: its element count (little-endian u64), then its data.
//!
//! Pickles 2 to 5 are decoded by candle's unpickler; the 10-byte magic number
//! (beyond what candle's LONG1 decoding handles) is checked byte for byte.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use candle_core::pickle::{Object, Stack};
use candle_core::{DType, Device, Tensor};

type Result<T> = std::result::Result<T, String>;

/// `pickle.dumps(0x1950a86a20f9469cfc6c, protocol=2)`.
const LEGACY_MAGIC: [u8; 16] = [
    0x80, 0x02, 0x8a, 0x0a, 0x6c, 0xfc, 0x9c, 0x46, 0xf9, 0x20, 0x6a, 0xa8, 0x50, 0x19, 0x2e, 0x00,
];
const LEGACY_PROTOCOL: i64 = 1001;

/// All tensors of the checkpoint, by name.
pub fn load(path: &Path, device: &Device) -> Result<HashMap<String, Tensor>> {
    let mut head = [0u8; 4];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .map_err(|e| e.to_string())?;
    if head == *b"PK\x03\x04" {
        let all = candle_core::pickle::read_all(path).map_err(|e| e.to_string())?;
        return all
            .into_iter()
            .map(|(n, t)| Ok((n, t.to_device(device).map_err(|e| e.to_string())?)))
            .collect();
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    load_legacy(&mut BufReader::with_capacity(1 << 20, file), device)
}

fn unpickle<R: BufRead>(r: &mut R) -> Result<Object> {
    let mut stack = Stack::empty();
    stack
        .read_loop(r)
        .map_err(|e| format!("pickle illisible : {e}"))?;
    stack.finalize().map_err(|e| e.to_string())
}

/// What a tensor needs from its storage.
struct View {
    key: String,
    dtype: DType,
    offset: usize,
    size: Vec<usize>,
    stride: Vec<usize>,
}

fn dims(o: Object) -> Result<Vec<usize>> {
    match o {
        Object::Tuple(v) | Object::List(v) => v
            .into_iter()
            .map(|d| {
                d.int_or_long()
                    .map(|d| d as usize)
                    .map_err(|o| format!("dimension {o:?}"))
            })
            .collect(),
        o => Err(format!("dimensions attendues, trouvé {o:?}")),
    }
}

fn storage_dtype(class: &str) -> Result<DType> {
    Ok(match class {
        "FloatStorage" => DType::F32,
        "HalfStorage" => DType::F16,
        "BFloat16Storage" => DType::BF16,
        "DoubleStorage" => DType::F64,
        "LongStorage" => DType::I64,
        "ByteStorage" => DType::U8,
        other => return Err(format!("type de stockage non pris en charge : {other}")),
    })
}

/// `torch._utils._rebuild_tensor_v2(storage, offset, size, stride, ...)`;
/// `None` for entries that are not tensors (`_metadata`).
fn view(value: Object) -> Result<Option<View>> {
    let Ok((callable, args)) = value.reduce() else {
        return Ok(None);
    };
    let (callable, args) = match callable {
        Object::Class {
            module_name,
            class_name,
        } if module_name == "torch._utils" && class_name == "_rebuild_parameter" => {
            let mut a = args.tuple().map_err(|o| format!("{o:?}"))?;
            a.remove(0).reduce().map_err(|o| format!("{o:?}"))?
        }
        c => (c, args),
    };
    match &callable {
        Object::Class {
            module_name,
            class_name,
        } if module_name == "torch._utils" && class_name == "_rebuild_tensor_v2" => {}
        _ => return Ok(None),
    }
    let mut a = args.tuple().map_err(|o| format!("{o:?}"))?;
    if a.len() < 4 {
        return Err("tenseur incomplet".into());
    }
    let stride = dims(a.remove(3))?;
    let size = dims(a.remove(2))?;
    let mut offset = a.remove(1).int_or_long().map_err(|o| format!("{o:?}"))? as usize;
    let id = a
        .remove(0)
        .persistent_load()
        .map_err(|o| format!("{o:?}"))?;
    let mut id = id.tuple().map_err(|o| format!("{o:?}"))?;
    if id.len() < 5 {
        return Err("identifiant de stockage incomplet".into());
    }
    // Legacy ids carry a 6th item: (view_key, view_offset, view_size) when the
    // storage is a slice of a bigger one, stored under the root key.
    if id.len() >= 6 {
        if let Object::Tuple(mut v) = id.remove(5) {
            if v.len() >= 2 {
                offset += v.remove(1).int_or_long().map_err(|o| format!("{o:?}"))? as usize;
            }
        }
    }
    let key = id.remove(2).unicode().map_err(|o| format!("clé {o:?}"))?;
    let (_, class) = id.remove(1).class().map_err(|o| format!("{o:?}"))?;
    Ok(Some(View {
        key,
        dtype: storage_dtype(&class)?,
        offset,
        size,
        stride,
    }))
}

fn load_legacy<R: BufRead>(r: &mut R, device: &Device) -> Result<HashMap<String, Tensor>> {
    let bad = || "format PyTorch inconnu (ni zip, ni ancien format)".to_string();
    let mut magic = [0u8; 15];
    r.read_exact(&mut magic).map_err(|_| bad())?;
    if magic[..] != LEGACY_MAGIC[..15] {
        return Err(bad());
    }
    if unpickle(r)?.int_or_long().ok() != Some(LEGACY_PROTOCOL) {
        return Err("version de l'ancien format PyTorch inattendue".into());
    }
    let sys_info = unpickle(r)?;
    let little = sys_info.dict().ok().and_then(|d| {
        d.into_iter()
            .find(|(k, _)| *k == Object::Unicode("little_endian".into()))
            .and_then(|(_, v)| v.bool().ok())
    });
    if little == Some(false) {
        return Err("modèle enregistré en big-endian".into());
    }

    let state = match unpickle(r)? {
        Object::Dict(d) => d,
        o => return Err(format!("dictionnaire de poids attendu, trouvé {o:?}")),
    };
    let mut views = Vec::new();
    for (name, value) in state {
        let Ok(name) = name.unicode() else { continue };
        if let Some(v) = view(value).map_err(|e| format!("{name} : {e}"))? {
            views.push((name, v));
        }
    }
    let keys = match unpickle(r)? {
        Object::List(k) => k
            .into_iter()
            .map(|k| k.unicode().map_err(|o| format!("clé de stockage {o:?}")))
            .collect::<Result<Vec<_>>>()?,
        o => return Err(format!("liste des stockages attendue, trouvé {o:?}")),
    };

    // Storages, in the order of the key list; their type comes from the tensors.
    let dtypes: HashMap<&str, DType> = views
        .iter()
        .map(|(_, v)| (v.key.as_str(), v.dtype))
        .collect();
    let mut storages: HashMap<String, Tensor> = HashMap::new();
    for key in &keys {
        let mut n = [0u8; 8];
        r.read_exact(&mut n)
            .map_err(|e| format!("stockage {key} : {e}"))?;
        let numel = u64::from_le_bytes(n) as usize;
        let dtype = dtypes.get(key.as_str()).copied().unwrap_or(DType::F32);
        let mut data = vec![0u8; numel * dtype.size_in_bytes()];
        r.read_exact(&mut data)
            .map_err(|_| format!("fichier tronqué (stockage {key})"))?;
        if dtypes.contains_key(key.as_str()) {
            let t = Tensor::from_raw_buffer(&data, dtype, &[numel], device)
                .map_err(|e| e.to_string())?;
            storages.insert(key.clone(), t);
        }
    }

    let mut out = HashMap::new();
    for (name, v) in views {
        let storage = storages
            .get(&v.key)
            .ok_or_else(|| format!("{name} : stockage {} absent", v.key))?;
        out.insert(
            name.clone(),
            materialize(storage, &v).map_err(|e| format!("{name} : {e}"))?,
        );
    }
    Ok(out)
}

/// The tensor described by `v` over its flat storage (any offset and strides).
fn materialize(storage: &Tensor, v: &View) -> Result<Tensor> {
    let numel: usize = v.size.iter().product();
    let len = storage.dims1().map_err(|e| e.to_string())?;
    let mut contiguous = vec![1usize; v.size.len()];
    for i in (0..v.size.len().saturating_sub(1)).rev() {
        contiguous[i] = contiguous[i + 1] * v.size[i + 1];
    }
    let shape = v.size.clone();
    if numel == 0 {
        return Tensor::zeros(shape, storage.dtype(), storage.device()).map_err(|e| e.to_string());
    }
    let last = v.offset
        + v.size
            .iter()
            .zip(&v.stride)
            .map(|(&s, &st)| (s - 1) * st)
            .sum::<usize>();
    if last >= len {
        return Err(format!("hors du stockage ({last} ≥ {len})"));
    }
    // Strides of size-1 dimensions do not matter.
    let same = v
        .size
        .iter()
        .zip(v.stride.iter().zip(&contiguous))
        .all(|(&s, (a, b))| s == 1 || a == b);
    let t = if same {
        storage.narrow(0, v.offset, numel)
    } else {
        // Strided view: gather the elements in row-major order.
        let mut idx = Vec::with_capacity(numel);
        let mut pos = vec![0usize; v.size.len()];
        for _ in 0..numel {
            idx.push(
                (v.offset + pos.iter().zip(&v.stride).map(|(p, s)| p * s).sum::<usize>()) as u32,
            );
            for d in (0..pos.len()).rev() {
                pos[d] += 1;
                if pos[d] < v.size[d] {
                    break;
                }
                pos[d] = 0;
            }
        }
        let idx = Tensor::from_vec(idx, numel, storage.device()).map_err(|e| e.to_string())?;
        storage.index_select(&idx, 0)
    };
    t.and_then(|t| t.reshape(shape)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_matches_torch() {
        // pickle.dumps(0x1950a86a20f9469cfc6c, protocol=2)
        assert_eq!(
            &LEGACY_MAGIC[..15],
            b"\x80\x02\x8a\x0al\xfc\x9cF\xf9 j\xa8P\x19."
        );
    }

    #[test]
    fn legacy_and_zip_checkpoints_hold_the_same_tensors() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/marian-tiny");
        let dev = Device::Cpu;
        let zip = load(&dir.join("pytorch_model.bin"), &dev).unwrap();
        let legacy = load(&dir.join("pytorch_model.legacy.bin"), &dev).unwrap();
        assert!(zip.len() > 50);
        assert_eq!(zip.len(), legacy.len());
        for (name, a) in &zip {
            let b = &legacy[name];
            assert_eq!(a.dims(), b.dims(), "{name}");
            let a: Vec<f32> = a.flatten_all().unwrap().to_vec1().unwrap();
            let b: Vec<f32> = b.flatten_all().unwrap().to_vec1().unwrap();
            assert_eq!(a, b, "{name}");
        }
    }

    #[test]
    fn rejects_unknown_and_truncated_files() {
        let dir = std::env::temp_dir().join(format!("crosstalk-pth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let junk = dir.join("junk.bin");
        std::fs::write(&junk, b"not a checkpoint at all").unwrap();
        assert!(load(&junk, &Device::Cpu)
            .unwrap_err()
            .contains("format PyTorch inconnu"));
        let src = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/marian-tiny/pytorch_model.legacy.bin");
        let data = std::fs::read(src).unwrap();
        let cut = dir.join("cut.bin");
        std::fs::write(&cut, &data[..data.len() - 100]).unwrap();
        assert!(load(&cut, &Device::Cpu).unwrap_err().contains("tronqué"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn strided_views_are_gathered() {
        let dev = Device::Cpu;
        let storage = Tensor::new(&[0f32, 1., 2., 3., 4., 5., 6., 7.], &dev).unwrap();
        // A 2x3 transposed view (stride 1, 2) starting at element 1.
        let v = View {
            key: "0".into(),
            dtype: DType::F32,
            offset: 1,
            size: vec![3, 2],
            stride: vec![1, 3],
        };
        let t = materialize(&storage, &v).unwrap();
        assert_eq!(
            t.to_vec2::<f32>().unwrap(),
            vec![vec![1., 4.], vec![2., 5.], vec![3., 6.]]
        );
        let v = View {
            key: "0".into(),
            dtype: DType::F32,
            offset: 2,
            size: vec![2, 2],
            stride: vec![2, 1],
        };
        assert_eq!(
            materialize(&storage, &v).unwrap().to_vec2::<f32>().unwrap(),
            vec![vec![2., 3.], vec![4., 5.]]
        );
        let v = View {
            key: "0".into(),
            dtype: DType::F32,
            offset: 6,
            size: vec![3],
            stride: vec![1],
        };
        assert!(materialize(&storage, &v).is_err());
    }
}

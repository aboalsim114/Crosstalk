//! Conversion of raw WASAPI buffers to interleaved `f32` samples.

use super::types::SampleFormat;

/// Picks the sample format from the fields of a `WAVEFORMATEX(TENSIBLE)`.
///
/// `float_subformat` is `Some(true)` for `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`,
/// `Some(false)` for `KSDATAFORMAT_SUBTYPE_PCM` and `None` for other sub formats.
pub fn classify(
    format_tag: u16,
    bits_per_sample: u16,
    valid_bits: Option<u16>,
    float_subformat: Option<bool>,
) -> Option<SampleFormat> {
    const WAVE_FORMAT_PCM: u16 = 1;
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    let is_float = match format_tag {
        WAVE_FORMAT_IEEE_FLOAT => true,
        WAVE_FORMAT_PCM => false,
        WAVE_FORMAT_EXTENSIBLE => float_subformat?,
        _ => return None,
    };
    match (is_float, bits_per_sample) {
        (true, 32) => Some(SampleFormat::F32),
        (false, 16) => Some(SampleFormat::I16),
        (false, 24) => Some(SampleFormat::I24),
        // 24-bit samples in 32-bit containers are left-aligned, so reading
        // them as i32 gives the right scale.
        (false, 32) if valid_bits.unwrap_or(32) >= 24 => Some(SampleFormat::I32),
        _ => None,
    }
}

/// Appends the samples of `bytes` (little-endian) to `out` as `f32` in -1..1.
pub fn to_f32(bytes: &[u8], format: SampleFormat, out: &mut Vec<f32>) {
    let step = format.bytes();
    out.reserve(bytes.len() / step);
    match format {
        SampleFormat::F32 => out.extend(
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        ),
        SampleFormat::I16 => out.extend(
            bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0),
        ),
        SampleFormat::I24 => out.extend(bytes.chunks_exact(3).map(|b| {
            let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
            v as f32 / 8_388_608.0
        })),
        SampleFormat::I32 => out.extend(
            bytes
                .chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0),
        ),
    }
}

/// Averages interleaved channels into one mono sample per frame.
pub fn mono_frames(samples: &[f32], channels: u16) -> impl Iterator<Item = f32> + '_ {
    let ch = channels.max(1) as usize;
    samples
        .chunks_exact(ch)
        .map(move |frame| frame.iter().sum::<f32>() / ch as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_mix_formats() {
        assert_eq!(classify(3, 32, None, None), Some(SampleFormat::F32));
        assert_eq!(classify(1, 16, None, None), Some(SampleFormat::I16));
        assert_eq!(
            classify(0xFFFE, 32, Some(32), Some(true)),
            Some(SampleFormat::F32)
        );
        assert_eq!(
            classify(0xFFFE, 24, Some(24), Some(false)),
            Some(SampleFormat::I24)
        );
        assert_eq!(
            classify(0xFFFE, 32, Some(24), Some(false)),
            Some(SampleFormat::I32)
        );
    }

    #[test]
    fn rejects_unknown_formats() {
        assert_eq!(classify(0xFFFE, 32, Some(32), None), None);
        assert_eq!(classify(2, 16, None, None), None); // ADPCM
        assert_eq!(classify(3, 64, None, None), None);
        assert_eq!(classify(1, 8, None, None), None);
    }

    #[test]
    fn converts_each_sample_format() {
        let mut out = Vec::new();
        to_f32(&0.5f32.to_le_bytes(), SampleFormat::F32, &mut out);
        to_f32(&i16::MIN.to_le_bytes(), SampleFormat::I16, &mut out);
        to_f32(&16_384i16.to_le_bytes(), SampleFormat::I16, &mut out);
        to_f32(&[0x00, 0x00, 0x40], SampleFormat::I24, &mut out); // +0.5
        to_f32(&[0x00, 0x00, 0x80], SampleFormat::I24, &mut out); // -1.0
        to_f32(&(i32::MAX / 2).to_le_bytes(), SampleFormat::I32, &mut out);
        let expected = [0.5, -1.0, 0.5, 0.5, -1.0, 0.5];
        for (a, b) in out.iter().zip(expected) {
            assert!((a - b).abs() < 1e-4, "{a} != {b}");
        }
    }

    #[test]
    fn ignores_trailing_partial_samples() {
        let mut out = Vec::new();
        to_f32(&[1, 2, 3], SampleFormat::I16, &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn downmixes_to_mono() {
        let stereo = [1.0, 0.0, -0.5, -0.5, 0.25, 0.75];
        let mono: Vec<f32> = mono_frames(&stereo, 2).collect();
        assert_eq!(mono, vec![0.5, -0.5, 0.5]);
        let same: Vec<f32> = mono_frames(&[0.1, 0.2], 1).collect();
        assert_eq!(same, vec![0.1, 0.2]);
    }
}

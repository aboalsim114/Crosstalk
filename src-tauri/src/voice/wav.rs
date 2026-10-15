//! The WAV files written by Piper: 16-bit PCM, mono, 16 or 22.05 kHz.

/// Mono samples in -1..1 and their sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Reads a RIFF/WAVE file of 16-bit PCM; several channels are averaged.
pub fn parse(bytes: &[u8]) -> Result<Pcm, String> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err("fichier audio invalide".into());
    }
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(bytes, pos + 4).ok_or("fichier audio tronqué")? as usize;
        let body = pos + 8;
        if id == b"fmt " {
            fmt = Some((
                u16_at(bytes, body).ok_or("format tronqué")?,
                u16_at(bytes, body + 2).ok_or("format tronqué")?,
                u32_at(bytes, body + 4).ok_or("format tronqué")?,
                u16_at(bytes, body + 14).ok_or("format tronqué")?,
            ));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = fmt.ok_or("format absent")?;
            if tag != 1 || bits != 16 || channels == 0 {
                return Err(format!(
                    "format audio non pris en charge ({tag}, {bits} bits)"
                ));
            }
            let end = (body + size).min(bytes.len());
            let ch = channels as usize;
            let samples = bytes[body..end]
                .chunks_exact(2 * ch)
                .map(|frame| {
                    frame
                        .chunks_exact(2)
                        .map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0)
                        .sum::<f32>()
                        / ch as f32
                })
                .collect();
            return Ok(Pcm { rate, samples });
        }
        pos = body + size + (size & 1);
    }
    Err("pas de données audio".into())
}

#[cfg(test)]
pub(crate) fn encode(rate: u32, samples: &[i16]) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_piper_output_and_refuses_other_files() {
        let pcm = parse(&encode(22_050, &[0, 16_384, -32_768])).unwrap();
        assert_eq!(pcm.rate, 22_050);
        assert_eq!(pcm.samples, vec![0.0, 0.5, -1.0]);
        assert!(parse(b"nope").is_err());
        let mut float = encode(16_000, &[1, 2]);
        float[20] = 3; // IEEE float tag
        assert!(parse(&float).is_err());
    }
}

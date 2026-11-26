// SPDX-License-Identifier: GPL-3.0-or-later
//! The optional click after an automatic fix: a short quiet tick made in memory (no sound
//! file, not the loud system beep).

use std::sync::OnceLock;

use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

const RATE: u32 = 22_050;

/// A WAV file: 35 ms of a 1.4 kHz tone that fades out fast, at a quarter of full volume.
fn tick() -> Vec<u8> {
    let n = RATE * 35 / 1000;
    let samples: Vec<i16> = (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let fade = (-t * 160.0).exp();
            ((2.0 * std::f32::consts::PI * 1400.0 * t).sin() * fade * 0.25 * i16::MAX as f32) as i16
        })
        .collect();
    let data = n * 2;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend(b"RIFF");
    w.extend((36 + data).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes()); // PCM
    w.extend(1u16.to_le_bytes()); // mono
    w.extend(RATE.to_le_bytes());
    w.extend((RATE * 2).to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend(data.to_le_bytes());
    samples.iter().for_each(|s| w.extend(s.to_le_bytes()));
    w
}

/// Play the click without waiting (the sound stays in memory for the whole run).
pub(super) fn click() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(tick);
    unsafe { PlaySoundW(wav.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_header() {
        let w = super::tick();
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize, w.len() - 44);
    }
}

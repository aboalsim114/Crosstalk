#!/usr/bin/env python3
"""Original soundtrack of the Crosstalk showcase, synthesised from scratch (numpy only).

Every cue is placed on the film's timeline (see STORYBOARD.md): drone and logo hit in
the intro, pulses for the problem, a light 120 BPM rhythm through the pipeline, the
music opening on the app reveal, a quiet break for the in-game moment with a chime at
each translation, then a rise and a resolution on the logo.

    python3 scripts/score.py            -> out/score.wav (48 kHz, stereo, 50 s)
"""
import os
import wave

import numpy as np

SR = 48000
DUR = 50.0
N = int(SR * DUR)
rng = np.random.default_rng(1234)
OUT = os.path.join(os.path.dirname(__file__), '..', 'out', 'score.wav')


def hz(note):
    names = {'C': -9, 'C#': -8, 'Db': -8, 'D': -7, 'D#': -6, 'Eb': -6, 'E': -5, 'F': -4, 'F#': -3, 'Gb': -3,
             'G': -2, 'G#': -1, 'Ab': -1, 'A': 0, 'A#': 1, 'Bb': 1, 'B': 2}
    n, o = note[:-1], int(note[-1])
    return 440.0 * 2 ** ((names[n] + (o - 4) * 12) / 12)


class Bus:
    def __init__(self):
        self.L = np.zeros(N)
        self.R = np.zeros(N)

    def add(self, t0, sig, pan=0.0, gain=1.0):
        i = int(t0 * SR)
        if i >= N:
            return
        if i < 0:
            sig = sig[-i:]
            i = 0
        sig = sig[: N - i] * gain
        l = np.cos((pan + 1) * np.pi / 4)
        r = np.sin((pan + 1) * np.pi / 4)
        self.L[i:i + len(sig)] += sig * l
        self.R[i:i + len(sig)] += sig * r


def tvec(d):
    return np.arange(int(d * SR)) / SR


def envelope(d, a, r, curve=2.0):
    t = tvec(d)
    e = np.minimum(1, t / max(a, 1e-4)) ** curve
    e *= np.clip((d - t) / max(r, 1e-4), 0, 1) ** curve
    return e


def pad_note(f, d, bright=0.5, a=1.6, r=2.2):
    t = tvec(d)
    out_l = np.zeros_like(t)
    out_r = np.zeros_like(t)
    for k in range(1, 8):
        amp = (1 / k ** (2.2 - bright)) * (1 if k < 5 else 0.4)
        for det, side in ((-4, 0), (4, 1)):
            ff = f * k * 2 ** (det / 1200)
            if ff > 9000:
                continue
            ph = rng.random() * 2 * np.pi
            lfo = 1 + 0.002 * np.sin(2 * np.pi * (0.13 + 0.05 * k) * t + ph)
            s = np.sin(2 * np.pi * ff * t * lfo + ph) * amp
            (out_l if side == 0 else out_r)[:] += s
    e = envelope(d, a, r)
    trem = 1 + 0.06 * np.sin(2 * np.pi * 0.21 * t)
    return out_l * e * trem, out_r * e * trem


def pad(bus, notes, t0, t1, gain=0.05, bright=0.5, a=1.6, r=2.2):
    for n in notes:
        # Each chord rings 0.8 s into the next one, so the harmony never drops out.
        l, rr = pad_note(hz(n), min(t1 + 0.8, DUR) - t0, bright, a, r)
        i = int(t0 * SR)
        m = min(len(l), N - i)
        bus.L[i:i + m] += l[:m] * gain
        bus.R[i:i + m] += rr[:m] * gain


def pluck(f, d=1.6, bright=1.0):
    t = tvec(d)
    s = np.zeros_like(t)
    for k in range(1, 7):
        s += np.sin(2 * np.pi * f * k * t) * np.exp(-t * (3 + k * 2.2 / bright)) / k ** 1.3
    s *= np.minimum(1, t / 0.004)
    return s


def chime(f, d=3.0):
    t = tvec(d)
    s = np.zeros_like(t)
    for ratio, amp, dec in ((1, 1, 1.4), (2.0, 0.5, 2.2), (3.01, 0.25, 3.5), (4.2, 0.12, 5)):
        s += amp * np.sin(2 * np.pi * f * ratio * t) * np.exp(-t * dec)
    return s * np.minimum(1, t / 0.003)


def kick(d=0.6, f0=110, f1=44):
    t = tvec(d)
    f = f1 + (f0 - f1) * np.exp(-t * 30)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) * np.exp(-t * 6.5) * np.minimum(1, t / 0.002)


def boom(d=4.0):
    t = tvec(d)
    f = 34 + 46 * np.exp(-t * 4)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) * np.exp(-t * 1.1) * np.minimum(1, t / 0.01)


def band_noise(d, fc, width):
    n = int(d * SR)
    spec = np.fft.rfft(rng.standard_normal(n))
    freqs = np.fft.rfftfreq(n, 1 / SR)
    spec *= np.exp(-0.5 * ((np.log2(np.maximum(freqs, 1)) - np.log2(fc)) / width) ** 2)
    x = np.fft.irfft(spec, n)
    return x / (np.abs(x).max() + 1e-9)


def tick(d=0.08, fc=6000):
    return band_noise(d, fc, 0.4) * np.exp(-tvec(d) * 60)


def whoosh(d=1.2, f_lo=250, f_hi=5000, rise=0.65):
    """Noise whose band sweeps up then down; envelope peaks at `rise` of the duration."""
    t = tvec(d)
    layers = [band_noise(d, f, 0.35) for f in np.geomspace(f_lo, f_hi, 6)]
    pos = np.where(t < rise * d, t / (rise * d), 1 - (t - rise * d) / ((1 - rise) * d)) * (len(layers) - 1)
    out = np.zeros_like(t)
    for i, lay in enumerate(layers):
        out += lay * np.clip(1 - np.abs(pos - i), 0, 1)
    e = np.where(t < rise * d, (t / (rise * d)) ** 2, ((d - t) / ((1 - rise) * d)) ** 1.5)
    return out * e


def riser(d=2.0):
    return whoosh(d, 200, 7000, rise=0.97)


def reverb(L, R, secs=2.8, wet=0.3, predelay=0.02):
    n = int(secs * SR)
    t = np.arange(n) / SR
    out = []
    for ch, seed in ((L, 1), (R, 2)):
        g = np.random.default_rng(seed)
        ir = g.standard_normal(n) * np.exp(-t * 6.9 / secs)
        # darker tail
        spec = np.fft.rfft(ir)
        freqs = np.fft.rfftfreq(n, 1 / SR)
        spec *= 1 / (1 + (freqs / 4500) ** 2)
        ir = np.fft.irfft(spec, n)
        ir = np.concatenate([np.zeros(int(predelay * SR)), ir])
        ir /= np.sqrt((ir ** 2).sum())
        size = 1 << int(np.ceil(np.log2(len(ch) + len(ir))))
        y = np.fft.irfft(np.fft.rfft(ch, size) * np.fft.rfft(ir, size), size)[: len(ch)]
        out.append(y)
    return L + out[0] * wet, R + out[1] * wet


def delay(L, R, secs=0.375, fb=0.35, wet=0.3):
    d = int(secs * SR)
    yl, yr = L.copy(), R.copy()
    tapl, tapr = L.copy(), R.copy()
    for k in range(1, 6):
        tapl = np.concatenate([np.zeros(d), tapl[:-d]]) * fb
        tapr = np.concatenate([np.zeros(d), tapr[:-d]]) * fb
        if k % 2:
            yl += tapr * wet * 2
        else:
            yr += tapl * wet * 2
    return yl, yr


pads, plucks, drums, fx, hits = Bus(), Bus(), Bus(), Bus(), Bus()

# ---- Harmony ---------------------------------------------------------------------
pad(pads, ['D2', 'A2'], 0.2, 5.6, gain=0.07, bright=0.2, a=2.5, r=1.5)
pad(pads, ['D3', 'A3', 'E4'], 1.2, 5.6, gain=0.03, bright=0.6, a=2.0, r=1.5)
pad(pads, ['D2', 'A2', 'F3', 'E4'], 5.0, 12.6, gain=0.045, bright=0.35, a=1.2, r=1.6)
pad(pads, ['Bb1', 'F2', 'D3', 'A3'], 12.2, 17.4, gain=0.045, bright=0.5)
pad(pads, ['G1', 'D2', 'Bb2', 'F3', 'A3'], 17.0, 22.6, gain=0.042, bright=0.5)
pad(pads, ['D2', 'A2', 'F3', 'C4', 'E4'], 22.2, 28.2, gain=0.05, bright=0.75, a=0.6)
pad(pads, ['Bb1', 'F2', 'D3', 'A3', 'C4'], 27.8, 33.4, gain=0.048, bright=0.7)
pad(pads, ['G2', 'D3', 'Bb3', 'A3'], 33.0, 37.4, gain=0.034, bright=0.4, a=0.8)
pad(pads, ['F2', 'C3', 'A3', 'G4'], 37.0, 42.4, gain=0.036, bright=0.55)
pad(pads, ['A1', 'E2', 'D3', 'G3', 'B3'], 42.0, 46.9, gain=0.05, bright=0.6, a=1.0, r=0.8)
pad(pads, ['D2', 'A2', 'F#3', 'A3', 'E4'], 46.7, 50.0, gain=0.06, bright=0.7, a=0.15, r=3.0)

# ---- Rhythm: 120 BPM through the pipeline, fuller on the app reveal ---------------
BEAT = 0.5
for i in range(int((33.0 - 12.0) / BEAT)):
    t = 12.0 + i * BEAT
    full = t >= 22.3
    if t < 21.8 or full:
        drums.add(t, kick(), gain=0.32 if full else 0.2)
    drums.add(t + BEAT / 2, tick(0.06, 7500), pan=0.25, gain=0.05 if full else 0.035)
    if full and i % 2 == 1:
        drums.add(t, band_noise(0.25, 2200, 0.6) * np.exp(-tvec(0.25) * 18), gain=0.05)
for i in range(int((45.4 - 42.0) / BEAT)):
    drums.add(42.0 + i * BEAT, kick(), gain=0.26)
    drums.add(42.0 + i * BEAT + BEAT / 2, tick(0.06, 7500), pan=-0.2, gain=0.04)

# Arpeggio (8th notes) following the chords.
ARP = [(12.0, 17.0, ['D4', 'F4', 'A4', 'D5', 'A4', 'F4']),
       (17.0, 22.2, ['D4', 'G4', 'Bb4', 'D5', 'Bb4', 'G4']),
       (22.3, 27.9, ['D4', 'F4', 'A4', 'C5', 'E5', 'C5', 'A4', 'F4']),
       (27.9, 33.0, ['D4', 'F4', 'Bb4', 'C5', 'D5', 'C5', 'Bb4', 'F4']),
       (42.0, 45.4, ['E4', 'A4', 'D5', 'G5', 'D5', 'A4'])]
for a, b, notes in ARP:
    k = 0
    t = a
    while t < b - 0.05:
        plucks.add(t, pluck(hz(notes[k % len(notes)]), 1.4, 1.2), pan=0.35 * np.sin(k * 0.9), gain=0.05)
        t += BEAT / 2
        k += 1

# ---- Effects and cues ------------------------------------------------------------
hits.add(3.0, boom(), gain=0.5)
for f, g, p in (('D5', 0.08, -0.3), ('A5', 0.05, 0.3), ('E6', 0.03, 0)):
    hits.add(3.02, chime(hz(f), 4.0), pan=p, gain=g)
fx.add(0.4, riser(2.7), gain=0.05)
fx.add(4.4, whoosh(1.4, 300, 5000, 0.55), pan=-0.2, gain=0.09)
# Callouts appearing (scene 2): soft glassy blips.
for i, n in enumerate(['A5', 'D6', 'F5', 'C6', 'E5', 'A5', 'D5']):
    fx.add(5.0 + i * 0.28 + 0.2, chime(hz(n), 1.2), pan=(-0.6 + i * 0.2), gain=0.022)
# Low heartbeat under the problem.
for t in np.arange(5.2, 11.2, 1.0):
    drums.add(t, kick(0.5, 70, 40), gain=0.16)
    drums.add(t + 0.22, kick(0.5, 64, 38), gain=0.09)
fx.add(10.4, riser(1.8), gain=0.08)
# Station arrivals (scene 3) and morphs.
for i, n in enumerate(['D5', 'F5', 'A5', 'C6', 'D6']):
    t = 12.6 + i * 2.0
    fx.add(t - 0.04, chime(hz(n), 2.0), pan=0.2, gain=0.035)
    fx.add(t - 1.0, whoosh(0.9, 400, 3500, 0.7), pan=-0.3, gain=0.04)
fx.add(15.9, riser(1.2), gain=0.03)
fx.add(17.9, riser(1.2), gain=0.03)
# Into the app.
fx.add(21.4, whoosh(1.2, 250, 6000, 0.75), gain=0.1)
hits.add(22.3, boom(3.0), gain=0.3)
hits.add(22.32, chime(hz('A4'), 3.0), gain=0.04)
for t in (25.6, 27.9, 30.3):
    fx.add(t - 0.25, whoosh(1.1, 300, 4000, 0.5), pan=0.4, gain=0.06)
fx.add(32.4, whoosh(1.6, 200, 3000, 0.6), gain=0.07)
# In game: one tick per spoken word, then a chime per overlay update.
for t in (34.05, 34.4, 34.7, 34.95, 35.3, 35.6, 35.85):
    fx.add(t, tick(0.05, 3000), pan=-0.4, gain=0.05)
fx.add(35.9, chime(hz('A5'), 2.0), pan=0.4, gain=0.04)
fx.add(36.97, chime(hz('D6'), 2.5), pan=0.4, gain=0.05)
fx.add(37.0, chime(hz('A6'), 2.5), pan=0.45, gain=0.025)
fx.add(38.6, chime(hz('F#6'), 2.0), pan=0.5, gain=0.035)
fx.add(39.6, riser(2.4), gain=0.06)
for i, t in enumerate((42.1, 42.18, 42.35, 42.4)):
    fx.add(t, whoosh(1.0, 300, 4500, 0.5), pan=(-0.7, -0.4, 0.4, 0.7)[i], gain=0.04)
fx.add(44.6, riser(2.1), gain=0.11)
hits.add(46.7, boom(4.5), gain=0.55)
for f, g, p in (('D5', 0.08, -0.3), ('F#5', 0.05, 0.25), ('A5', 0.05, 0.4), ('E6', 0.03, -0.1)):
    hits.add(46.73, chime(hz(f), 4.5), pan=p, gain=g)

# ---- Mix -------------------------------------------------------------------------
pl, pr = reverb(pads.L, pads.R, 3.5, 0.35)
ql, qr = delay(plucks.L, plucks.R, 0.375, 0.4, 0.25)
ql, qr = reverb(ql, qr, 2.5, 0.35)
dl, dr = reverb(drums.L, drums.R, 1.2, 0.12)
fl, fr = reverb(fx.L, fx.R, 3.0, 0.5)
hl, hr = reverb(hits.L, hits.R, 4.5, 0.55)
L = pl + ql + dl + fl + hl
R = pr + qr + dr + fr + hr

t = np.arange(N) / SR
fade = np.clip(t / 0.3, 0, 1) * np.clip((DUR - t) / 1.4, 0, 1) ** 1.5
L *= fade
R *= fade
# Gentle glue: soft saturation then normalise around -16 dBFS RMS, peak under -1 dBFS.
for arr in (L, R):
    arr[:] = np.tanh(arr * 1.6) / 1.6
rms = np.sqrt((L ** 2 + R ** 2).mean() / 2)
gain = 10 ** (-16 / 20) / max(rms, 1e-9)
peak = max(np.abs(L).max(), np.abs(R).max()) * gain
if peak > 10 ** (-1 / 20):
    gain *= 10 ** (-1 / 20) / peak
L *= gain
R *= gain

os.makedirs(os.path.dirname(OUT), exist_ok=True)
data = (np.stack([L, R], axis=1) * 32767).astype('<i2')
with wave.open(OUT, 'wb') as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(data.tobytes())
print('written', os.path.normpath(OUT), f'rms {20 * np.log10(np.sqrt((L ** 2).mean()) + 1e-9):.1f} dBFS',
      f'peak {20 * np.log10(max(np.abs(L).max(), np.abs(R).max())):.1f} dBFS')

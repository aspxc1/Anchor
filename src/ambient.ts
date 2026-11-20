import { useEffect, useState } from "react";
import type { Audio, Snapshot } from "./session";

type Noise = Audio["noise"];

interface Voice {
  noise: Noise;
  source: AudioBufferSourceNode;
  gain: GainNode;
}

const FADE = 0.24;

class AmbientAudio {
  private readonly context = new AudioContext();
  private readonly buffers = new Map<Noise, AudioBuffer>();
  private voice: Voice | undefined;

  prepare() {
    return this.context.resume();
  }

  private buffer(noise: Noise) {
    const existing = this.buffers.get(noise);
    if (existing) return existing;
    const frames = this.context.sampleRate * 8;
    const overlap = Math.round(this.context.sampleRate * 0.08);
    const signal = new Float32Array(frames + overlap);
    let brown = 0;
    let b0 = 0, b1 = 0, b2 = 0, b3 = 0, b4 = 0, b5 = 0, b6 = 0;
    for (let i = 0; i < signal.length; i++) {
      const white = 2 * Math.random() - 1;
      if (noise === "brown") {
        brown = (brown + 0.02 * white) / 1.02;
        signal[i] = brown * 3.5;
      } else {
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.96900 * b2 + white * 0.1538520;
        b3 = 0.86650 * b3 + white * 0.3104856;
        b4 = 0.55000 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.0168980;
        signal[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362) * 0.11;
        b6 = white * 0.115926;
      }
    }
    const buffer = this.context.createBuffer(1, frames, this.context.sampleRate);
    const samples = buffer.getChannelData(0);
    samples.set(signal.subarray(0, frames));
    for (let i = 0; i < overlap; i++) {
      const mix = i / overlap;
      samples[i] = signal[frames + i]! * (1 - mix) + signal[i]! * mix;
    }
    this.buffers.set(noise, buffer);
    return buffer;
  }

  apply(audio: Audio | undefined) {
    const now = this.context.currentTime;
    if (this.voice && (!audio || this.voice.noise !== audio.noise)) {
      const previous = this.voice;
      previous.gain.gain.cancelAndHoldAtTime(now);
      previous.gain.gain.linearRampToValueAtTime(0, now + FADE);
      previous.source.stop(now + FADE);
      this.voice = undefined;
    }
    if (!audio) return;
    if (!this.voice) {
      const source = this.context.createBufferSource();
      source.loop = true;
      source.buffer = this.buffer(audio.noise);
      const filter = this.context.createBiquadFilter();
      filter.type = "highpass";
      filter.frequency.value = audio.noise === "brown" ? 38 : 30;
      const gain = this.context.createGain();
      gain.gain.value = 0;
      source.connect(filter).connect(gain).connect(this.context.destination);
      source.onended = () => { source.disconnect(); filter.disconnect(); gain.disconnect(); };
      source.start(now);
      this.voice = { source, gain, noise: audio.noise };
    }
    this.voice.gain.gain.cancelAndHoldAtTime(now);
    this.voice.gain.gain.linearRampToValueAtTime((audio.volume / 100) ** 2 * 0.75, now + FADE);
  }
}

let ambient: AmbientAudio | undefined;

export async function prepareAudio() {
  ambient ??= new AmbientAudio();
  await ambient.prepare();
}

export function useAmbientAudio(snapshot: Snapshot | undefined) {
  const [error, setError] = useState<string>();
  const enabled = snapshot?.phase === "ActiveFocus" && snapshot.preferences.audio.enabled;
  const noise = snapshot?.preferences.audio.noise ?? "brown";
  const volume = snapshot?.preferences.audio.volume ?? 22;

  useEffect(() => {
    if (!ambient) return;
    try {
      ambient.apply(enabled ? { enabled, noise, volume } : undefined);
      setError(undefined);
    } catch (reason) {
      console.error(reason);
      setError("Ambient audio is unavailable. Try starting a new session.");
    }
  }, [enabled, noise, volume]);

  return error;
}

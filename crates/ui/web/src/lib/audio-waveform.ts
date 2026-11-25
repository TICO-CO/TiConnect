import type { Result } from './result';
import { attempt, attemptAsync, err, ok } from './result';

export const VOICE_DECODE_BARS = 96;
export const VOICE_MIN_PEAK = 0.06;

export interface VoiceWaveform {
    peaks: number[];
    duration: number;
}

export type VoiceWaveformFailure = { type: 'DECODE_FAILED' } | { type: 'UNSUPPORTED_BROWSER' };

export const VoiceWaveformFailures = {
    decodeFailed: (): VoiceWaveformFailure => ({ type: 'DECODE_FAILED' }),
    unsupportedBrowser: (): VoiceWaveformFailure => ({ type: 'UNSUPPORTED_BROWSER' }),
};

type WindowWithWebkitAudio = Window & { webkitAudioContext?: typeof AudioContext };

export function formatVoiceTime(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) {
        return '00:00';
    }
    const total = Math.floor(seconds);
    const minutes = Math.floor(total / 60);
    const rest = total % 60;
    return `${minutes.toString().padStart(2, '0')}:${rest.toString().padStart(2, '0')}`;
}

export function peaksFromSamples(samples: ArrayLike<number>, barCount: number): number[] {
    if (barCount <= 0 || samples.length === 0) {
        return [];
    }
    const peaks = Array.from({ length: barCount }, (_unused, index) => {
        const start = Math.floor((index * samples.length) / barCount);
        const end = Math.max(start + 1, Math.floor(((index + 1) * samples.length) / barCount));
        let energy = 0;
        for (let offset = start; offset < end; offset += 1) {
            const sample = samples[offset] ?? 0;
            energy += sample * sample;
        }
        return Math.sqrt(energy / (end - start));
    });
    const loudest = peaks.reduce((maximum, peak) => Math.max(maximum, peak), 0);
    if (loudest <= 0) {
        return peaks.map(() => 0);
    }
    return peaks.map((peak) => Math.min(1, peak / loudest));
}

export function resampleWaveform(history: readonly number[], barCount: number): number[] {
    if (barCount <= 0) {
        return [];
    }
    if (history.length === 0) {
        return Array.from({ length: barCount }, () => 0);
    }
    if (history.length === 1) {
        return Array.from({ length: barCount }, () => history[0] ?? 0);
    }
    return Array.from({ length: barCount }, (_unused, index) => {
        const position = (index * (history.length - 1)) / Math.max(1, barCount - 1);
        const base = Math.floor(position);
        const next = Math.min(history.length - 1, base + 1);
        const fraction = position - base;
        const first = history[base] ?? 0;
        const second = history[next] ?? 0;
        return first + (second - first) * fraction;
    });
}

export async function decodeVoiceWaveform(
    audioBytes: ArrayBuffer,
    barCount: number,
): Promise<Result<VoiceWaveform, VoiceWaveformFailure>> {
    const globalAudio = typeof AudioContext === 'undefined' ? undefined : AudioContext;
    const audioWindow = window as WindowWithWebkitAudio;
    const Context = globalAudio ?? audioWindow.webkitAudioContext;
    if (Context === undefined) {
        return err(VoiceWaveformFailures.unsupportedBrowser());
    }
    const contextResult = attempt<AudioContext>(() => new Context());
    if (!contextResult.ok) {
        return err(VoiceWaveformFailures.unsupportedBrowser());
    }
    const context = contextResult.value;
    const decodeResult = await attemptAsync<AudioBuffer>(() =>
        context.decodeAudioData(audioBytes.slice(0)),
    );
    if (!decodeResult.ok) {
        void context.close();
        return err(VoiceWaveformFailures.decodeFailed());
    }
    const audioBuffer = decodeResult.value;
    const waveform: VoiceWaveform = {
        peaks: peaksFromSamples(audioBuffer.getChannelData(0), barCount),
        duration: audioBuffer.duration,
    };
    void context.close();
    return ok(waveform);
}

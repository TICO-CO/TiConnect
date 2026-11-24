import { describe, expect, it, vi } from 'vitest';

import {
    decodeVoiceWaveform,
    formatVoiceTime,
    peaksFromSamples,
    resampleWaveform,
    VOICE_DECODE_BARS,
    VoiceWaveformFailures,
} from './audio-waveform';

describe('formatVoiceTime', () => {
    it('formats minutes and seconds', () => {
        expect(formatVoiceTime(0)).toBe('00:00');
        expect(formatVoiceTime(4)).toBe('00:04');
        expect(formatVoiceTime(65)).toBe('01:05');
    });

    it('falls back for invalid input', () => {
        expect(formatVoiceTime(Number.NaN)).toBe('00:00');
        expect(formatVoiceTime(-3)).toBe('00:00');
    });
});

describe('peaksFromSamples', () => {
    it('returns one normalized peak per bar', () => {
        const samples = [0, 0.5, 1, 0.5, 0, -0.5, -1, -0.5];
        const peaks = peaksFromSamples(samples, 4);
        expect(peaks).toHaveLength(4);
        expect(Math.max(...peaks)).toBeCloseTo(1);
        expect(Math.min(...peaks)).toBeGreaterThanOrEqual(0);
    });

    it('maps silence to flat peaks', () => {
        expect(peaksFromSamples([0, 0, 0, 0], 2)).toEqual([0, 0]);
    });

    it('returns no peaks for empty input', () => {
        expect(peaksFromSamples([], 4)).toEqual([]);
        expect(peaksFromSamples([0.5], 0)).toEqual([]);
    });

    it('uses the decode bar count', () => {
        expect(peaksFromSamples(new Array(300).fill(0.5), VOICE_DECODE_BARS)).toHaveLength(
            VOICE_DECODE_BARS,
        );
    });
});

describe('resampleWaveform', () => {
    it('fits any bar count while keeping order', () => {
        expect(resampleWaveform([0, 0.5, 1], 5)).toHaveLength(5);
        expect(resampleWaveform([0, 0.5, 1], 5)[0]).toBeCloseTo(0);
        expect(resampleWaveform([0, 0.5, 1], 5)[4]).toBeCloseTo(1);
    });

    it('downsamples long histories', () => {
        const history = Array.from({ length: 100 }, (_unused, index) => index / 99);
        const bars = resampleWaveform(history, 10);
        expect(bars).toHaveLength(10);
        for (let index = 1; index < bars.length; index += 1) {
            expect(bars[index] ?? 0).toBeGreaterThanOrEqual(bars[index - 1] ?? 0);
        }
    });

    it('handles degenerate inputs', () => {
        expect(resampleWaveform([], 4)).toEqual([0, 0, 0, 0]);
        expect(resampleWaveform([0.5], 3)).toEqual([0.5, 0.5, 0.5]);
        expect(resampleWaveform([0.5], 0)).toEqual([]);
        expect(resampleWaveform([0, 1], 1)).toEqual([0]);
    });
});

describe('decodeVoiceWaveform', () => {
    it('reports unsupported browsers without an audio context', async () => {
        vi.stubGlobal('AudioContext', undefined);
        const result = await decodeVoiceWaveform(new ArrayBuffer(8), 4);
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toEqual(VoiceWaveformFailures.unsupportedBrowser());
    });

    it('reports undecodable audio', async () => {
        const close = vi.fn(async () => undefined);
        const decodeAudioData = vi.fn(() => Promise.reject(new Error('nope')));
        class FakeAudioContext {
            decodeAudioData = decodeAudioData;
            close = close;
        }
        vi.stubGlobal('AudioContext', FakeAudioContext);
        const result = await decodeVoiceWaveform(new ArrayBuffer(8), 4);
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toEqual(VoiceWaveformFailures.decodeFailed());
        expect(close).toHaveBeenCalled();
    });

    it('decodes peaks from channel data', async () => {
        const close = vi.fn(async () => undefined);
        const channel = new Float32Array([0, 0.5, 1, 0.5, 0, -1]);
        const decodeAudioData = vi.fn(async () => ({
            duration: 6,
            getChannelData: () => channel,
        }));
        class FakeAudioContext {
            decodeAudioData = decodeAudioData;
            close = close;
        }
        vi.stubGlobal('AudioContext', FakeAudioContext);
        const result = await decodeVoiceWaveform(new ArrayBuffer(8), 3);
        expect(result.ok).toBe(true);
        if (!result.ok) {
            return;
        }
        expect(result.value.duration).toBe(6);
        expect(result.value.peaks).toHaveLength(3);
        expect(Math.max(...result.value.peaks)).toBeCloseTo(1);
    });
});

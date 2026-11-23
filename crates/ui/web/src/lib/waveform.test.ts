import { describe, expect, it } from 'vitest';

import {
    interpolatedSample,
    pushWaveSample,
    WAVE_BAR_COUNT,
    WAVE_BAR_LENGTH,
    WAVE_HISTORY_SIZE,
    WAVE_MIN_LENGTH,
    waveBars,
} from './waveform';

describe('pushWaveSample', () => {
    it('appends and clamps the level', () => {
        expect(pushWaveSample([0.5], 2)).toEqual([0.5, 1]);
        expect(pushWaveSample([0.5], -2)).toEqual([0.5, 0]);
        expect(pushWaveSample([0.5], Number.NaN)).toEqual([0.5, 0]);
    });

    it('keeps only the newest samples', () => {
        const history = Array.from({ length: WAVE_HISTORY_SIZE }, () => 0.5);
        const next = pushWaveSample(history, 1);
        expect(next).toHaveLength(WAVE_HISTORY_SIZE);
        expect(next.at(-1)).toBe(1);
        expect(next[0]).toBe(0.5);
    });
});

describe('interpolatedSample', () => {
    it('returns zero for an empty history', () => {
        expect(interpolatedSample([], 3)).toBe(0);
    });

    it('interpolates between neighbouring samples', () => {
        expect(interpolatedSample([0, 1], 0.5)).toBe(0.5);
    });

    it('wraps positions around the history', () => {
        expect(interpolatedSample([0, 1], -0.5)).toBe(0.5);
        expect(interpolatedSample([0, 1], 2)).toBe(0);
    });
});

describe('waveBars', () => {
    const silentHistory = Array.from({ length: WAVE_HISTORY_SIZE }, () => 0);

    it('creates evenly spaced radial bars', () => {
        const bars = waveBars(silentHistory, 0);
        expect(bars).toHaveLength(WAVE_BAR_COUNT);
        expect(bars[0]?.angle).toBe(0);
        expect(bars[WAVE_BAR_COUNT / 2]?.angle).toBe(180);
    });

    it('keeps silent bars at the minimum length', () => {
        const bars = waveBars(silentHistory, 0);
        for (const bar of bars) {
            expect(bar.height).toBeCloseTo(WAVE_MIN_LENGTH);
            expect(bar.opacity).toBeCloseTo(0.4);
        }
    });

    it('grows bars for loud samples', () => {
        const loudHistory = Array.from({ length: WAVE_HISTORY_SIZE }, () => 1);
        const bars = waveBars(loudHistory, 0);
        for (const bar of bars) {
            expect(bar.height).toBeCloseTo(WAVE_MIN_LENGTH + WAVE_BAR_LENGTH);
            expect(bar.opacity).toBeCloseTo(1);
        }
    });

    it('moves samples around the ring as scroll advances', () => {
        const history = [1, 0];
        const before = waveBars(history, 0);
        const after = waveBars(history, 1);
        expect(before[0]?.height).toBeGreaterThan(WAVE_MIN_LENGTH);
        expect(after[0]?.height).toBeLessThan(before[0]?.height ?? 0);
        expect(after[WAVE_BAR_COUNT - 1]?.height).toBeGreaterThan(WAVE_MIN_LENGTH);
    });
});

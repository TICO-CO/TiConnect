export const WAVE_SIZE = 300;
export const WAVE_BAR_COUNT = 56;
export const WAVE_BAR_WIDTH = 4;
export const WAVE_INNER_RADIUS = 92;
export const WAVE_BAR_LENGTH = 50;
export const WAVE_MIN_LENGTH = 6;
export const WAVE_HISTORY_SIZE = 64;
export const WAVE_STEP_MS = 50;

export interface WaveBar {
    angle: number;
    x: number;
    y: number;
    height: number;
    opacity: number;
    leading: boolean;
}

export function pushWaveSample(history: readonly number[], level: number): number[] {
    const clamped = Number.isFinite(level) ? Math.min(1, Math.max(0, level)) : 0;
    return [...history, clamped].slice(-WAVE_HISTORY_SIZE);
}

export function interpolatedSample(history: readonly number[], position: number): number {
    if (history.length === 0) {
        return 0;
    }
    const length = history.length;
    const wrapped = ((position % length) + length) % length;
    const base = Math.floor(wrapped);
    const fraction = wrapped - base;
    const first = history[base] ?? 0;
    const second = history[(base + 1) % length] ?? 0;
    return first + (second - first) * fraction;
}

export function waveBars(history: readonly number[]): WaveBar[] {
    const center = WAVE_SIZE / 2;
    const x = (WAVE_SIZE - WAVE_BAR_WIDTH) / 2;
    return Array.from({ length: WAVE_BAR_COUNT }, (_unused, index) => {
        const angle = (360 * index) / WAVE_BAR_COUNT;
        const sample = interpolatedSample(history, history.length - 1 - index);
        const height = WAVE_MIN_LENGTH + WAVE_BAR_LENGTH * Math.pow(sample, 1.35);
        return {
            angle,
            x,
            y: center - WAVE_INNER_RADIUS - height,
            height,
            opacity: 0.4 + 0.6 * sample,
            leading: index === 0,
        };
    });
}

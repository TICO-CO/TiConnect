import { act, render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import { WAVE_BAR_COUNT, WAVE_MIN_LENGTH } from '../lib';
import CallWaveform from './CallWaveform.svelte';

function barHeights(container: HTMLElement): number[] {
    return Array.from(container.querySelectorAll('rect')).map((rect) =>
        Number(rect.getAttribute('height')),
    );
}

describe('CallWaveform', () => {
    it('renders one bar per radial slot with a cursor on the newest bar', () => {
        const { container } = render(CallWaveform, { level: 0.4 });
        expect(container.querySelectorAll('rect')).toHaveLength(WAVE_BAR_COUNT);
        const cursor = container.querySelector('circle');
        expect(cursor?.getAttribute('fill')).toBe('var(--color-accent)');
    });

    it('uses the warning tone while reconnecting', () => {
        const { container } = render(CallWaveform, { level: 0.4, tone: 'warning' });
        const firstBar = container.querySelector('rect');
        expect(firstBar?.getAttribute('fill')).toBe('var(--color-warning)');
    });

    it('grows bars when the audio level rises', async () => {
        const frames: FrameRequestCallback[] = [];
        vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
            frames.push(callback);
            return frames.length;
        });
        vi.stubGlobal('cancelAnimationFrame', () => undefined);
        const { container, rerender } = render(CallWaveform, { level: 0 });
        const runFrame = async (time: number) => {
            const callback = frames.shift();
            await act(() => {
                callback?.(time);
            });
        };

        let time = 1_000;
        for (let step = 0; step < 10; step += 1) {
            await runFrame(time);
            time += 50;
        }
        expect(Math.max(...barHeights(container))).toBeCloseTo(WAVE_MIN_LENGTH);

        await rerender({ level: 1 });
        for (let step = 0; step < 10; step += 1) {
            await runFrame(time);
            time += 50;
        }
        expect(Math.max(...barHeights(container))).toBeGreaterThan(WAVE_MIN_LENGTH);
    });
});

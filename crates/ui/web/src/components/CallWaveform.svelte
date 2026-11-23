<script lang="ts">
    import { onMount } from 'svelte';

    import {
        pushWaveSample,
        WAVE_BAR_WIDTH,
        WAVE_HISTORY_SIZE,
        WAVE_SIZE,
        WAVE_STEP_MS,
        type WaveBar,
        waveBars,
    } from '../lib';

    interface Props {
        level?: number;
        tone?: 'normal' | 'warning';
        demo?: boolean;
    }

    let { level = 0, tone = 'normal', demo = false }: Props = $props();
    const initialHistory = Array.from({ length: WAVE_HISTORY_SIZE }, () => 0);
    let history = $state<number[]>(initialHistory);
    let bars = $state<WaveBar[]>(waveBars(initialHistory, 0));
    let lastLevel = 0;
    const fill = $derived(tone === 'warning' ? 'var(--color-warning)' : 'var(--color-accent)');

    function demoLevel(seconds: number): number {
        const speech = Math.abs(Math.sin(seconds * 2.3));
        const syllables = Math.abs(Math.sin(seconds * 9.1 + 0.6));
        const swell = 0.55 + 0.45 * Math.abs(Math.sin(seconds * 0.7));
        const detail = Math.abs(Math.sin(seconds * 17.3));
        return Math.min(1, 0.18 + 0.62 * speech * syllables * swell + 0.12 * detail);
    }

    $effect(() => {
        if (demo || level === lastLevel) {
            return;
        }
        lastLevel = level;
        history = pushWaveSample(history, level);
    });

    onMount(() => {
        if (demo) {
            history = Array.from({ length: WAVE_HISTORY_SIZE }, (_unused, index) =>
                demoLevel(((index - WAVE_HISTORY_SIZE) * WAVE_STEP_MS) / 1000),
            );
        }
        let frame = 0;
        let previous = 0;
        let sampleAccumulator = 0;
        let scroll = 0;
        const tick = (time: number) => {
            const elapsed = previous === 0 ? 0 : Math.min(time - previous, 250);
            previous = time;
            scroll += elapsed / WAVE_STEP_MS;
            if (demo) {
                sampleAccumulator += elapsed;
                if (sampleAccumulator >= WAVE_STEP_MS) {
                    sampleAccumulator %= WAVE_STEP_MS;
                    history = pushWaveSample(history, demoLevel(time / 1000));
                }
            }
            bars = waveBars(history, scroll);
            frame = requestAnimationFrame(tick);
        };
        frame = requestAnimationFrame(tick);
        return () => cancelAnimationFrame(frame);
    });
</script>

<svg
    class="pointer-events-none absolute inset-0 size-[300px]"
    viewBox={`0 0 ${WAVE_SIZE} ${WAVE_SIZE}`}
    aria-hidden="true"
>
    {#each bars as bar, barIndex (barIndex)}
        <rect
            x={bar.x}
            y={bar.y}
            width={WAVE_BAR_WIDTH}
            height={bar.height}
            rx={WAVE_BAR_WIDTH / 2}
            {fill}
            opacity={bar.opacity}
            transform={`rotate(${bar.angle} ${WAVE_SIZE / 2} ${WAVE_SIZE / 2})`}
        />
    {/each}
</svg>

<script lang="ts" module>
    let activeStop: (() => void) | null = null;
</script>

<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState, Message } from '../lib';
    import {
        attemptAsync,
        decodeVoiceWaveform,
        formatVoiceTime,
        resampleWaveform,
        VOICE_DECODE_BARS,
        VOICE_MIN_PEAK,
    } from '../lib';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
        message: Message;
    }

    let { app, message }: Props = $props();

    const VOICE_SKELETON = [0.26, 0.47, 0.68, 0.37, 0.16, 0.58, 0.79, 0.47, 0.26, 0.42, 0.63, 0.32];
    const VOICE_LANE_HEIGHT = 44;
    const VOICE_BAR_MIN = 6;
    const VOICE_BAR_PITCH = 6;

    let audioElement: HTMLAudioElement | null = $state(null);
    let waveLane: HTMLElement | null = $state(null);
    let laneWidth = $state(0);
    let sourceUrl = $state('');
    let peaks = $state<number[]>([]);
    let duration = $state(0);
    let position = $state(0);
    let playing = $state(false);
    let loadingVoice = $state(true);
    let voiceFailed = $state(false);
    let loadStarted = false;
    let animationFrame = 0;

    const transfer = $derived(app.transferFor(message.attachment_hash));
    const ready = $derived(sourceUrl !== '' && peaks.length > 0);
    const progress = $derived(duration > 0 ? Math.min(1, position / duration) : 0);
    const timeLabel = $derived(
        playing || position > 0 ? formatVoiceTime(position) : formatVoiceTime(duration),
    );
    const statusLabel = $derived.by(() => {
        if (transfer !== null && !transfer.done) {
            if (transfer.total > 0) {
                const percent = Math.min(100, Math.round((transfer.bytes / transfer.total) * 100));
                return `Transferring · ${percent}%`;
            }
            return 'Transferring…';
        }
        return 'Loading voice message…';
    });
    const barCount = $derived(
        laneWidth > 0 ? Math.max(1, Math.floor(laneWidth / VOICE_BAR_PITCH)) : 1,
    );
    const displayHeights = $derived(
        (ready
            ? resampleWaveform(peaks, barCount)
            : resampleWaveform(VOICE_SKELETON, barCount)
        ).map((peak) => barHeight(peak)),
    );

    function barHeight(peak: number): number {
        return VOICE_BAR_MIN + Math.max(VOICE_MIN_PEAK, peak) * (VOICE_LANE_HEIGHT - VOICE_BAR_MIN);
    }

    function stopSelf(): void {
        audioElement?.pause();
    }

    function markActive(): void {
        activeStop?.();
        activeStop = stopSelf;
    }

    function clearActive(): void {
        if (activeStop === stopSelf) {
            activeStop = null;
        }
    }

    function syncFromElement(): void {
        if (audioElement === null) {
            return;
        }
        position = audioElement.currentTime;
        if (duration === 0 && Number.isFinite(audioElement.duration)) {
            duration = audioElement.duration;
        }
    }

    function syncPosition(): void {
        syncFromElement();
        animationFrame = requestAnimationFrame(syncPosition);
    }

    function stopSync(): void {
        cancelAnimationFrame(animationFrame);
    }

    function handlePlay(): void {
        playing = true;
        markActive();
        stopSync();
        animationFrame = requestAnimationFrame(syncPosition);
    }

    function handlePause(): void {
        playing = false;
        clearActive();
        stopSync();
    }

    async function togglePlayback(): Promise<void> {
        const element = audioElement;
        if (element === null) {
            return;
        }
        if (playing) {
            element.pause();
            return;
        }
        const playResult = await attemptAsync(() => element.play());
        if (!playResult.ok) {
            playing = false;
        }
    }

    function seekVoice(event: Event): void {
        const slider = event.currentTarget as HTMLInputElement;
        const next = Number(slider.value);
        if (audioElement !== null && Number.isFinite(next)) {
            audioElement.currentTime = next;
        }
        position = next;
    }

    async function loadVoice(transferFailure: string): Promise<void> {
        if (loadStarted) {
            return;
        }
        loadStarted = true;
        if (transferFailure !== '') {
            loadingVoice = false;
            voiceFailed = true;
            return;
        }
        const url = await app.attachmentUrl(message.attachment_hash);
        if (url === '') {
            loadingVoice = false;
            voiceFailed = true;
            return;
        }
        sourceUrl = url;
        const responseResult = await attemptAsync(() => fetch(url));
        if (!responseResult.ok) {
            loadingVoice = false;
            voiceFailed = true;
            return;
        }
        const bytesResult = await attemptAsync(() => responseResult.value.arrayBuffer());
        if (!bytesResult.ok) {
            loadingVoice = false;
            voiceFailed = true;
            return;
        }
        const waveResult = await decodeVoiceWaveform(bytesResult.value, VOICE_DECODE_BARS);
        if (!waveResult.ok) {
            loadingVoice = false;
            voiceFailed = true;
            return;
        }
        peaks = waveResult.value.peaks;
        duration = waveResult.value.duration;
        loadingVoice = false;
    }

    function retryVoice(): void {
        loadStarted = false;
        sourceUrl = '';
        peaks = [];
        duration = 0;
        position = 0;
        voiceFailed = false;
        loadingVoice = true;
        void loadVoice('');
    }

    $effect(() => {
        if (transfer !== null && !transfer.done) {
            return;
        }
        void loadVoice(transfer?.failed ?? '');
    });

    $effect(() => {
        const gain = app.messageOutputVolume(message.sender_id);
        if (audioElement !== null) {
            audioElement.volume = gain;
        }
    });

    onMount(() => {
        if (waveLane !== null && typeof ResizeObserver !== 'undefined') {
            const observer = new ResizeObserver((entries) => {
                laneWidth = entries[0]?.contentRect.width ?? 0;
            });
            observer.observe(waveLane);
        }
        return () => {
            stopSync();
            audioElement?.pause();
            clearActive();
        };
    });
</script>

{#if voiceFailed}
    <div
        class="flex w-[340px] max-w-full items-center gap-3 rounded-md border border-line bg-surface px-3.5 py-3"
    >
        <button
            class="flex size-9 shrink-0 items-center justify-center rounded-pill border border-error"
            onclick={retryVoice}
            aria-label="Retry voice message"
        >
            <Icon name="retry" size={16} color="var(--color-error)" />
        </button>
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <strong class="text-[14px]">Voice message unavailable</strong>
            <span class="text-xs text-ink-3">Tap retry to load it again</span>
        </span>
    </div>
{:else}
    <div
        class="flex w-[340px] max-w-full items-center gap-3 rounded-md border border-line bg-surface px-3.5 py-3"
    >
        <button
            class="flex size-9 shrink-0 items-center justify-center rounded-pill border border-line-strong disabled:opacity-50"
            onclick={() => void togglePlayback()}
            disabled={!ready}
            aria-label={playing ? 'Pause voice message' : 'Play voice message'}
        >
            <Icon name={playing ? 'pause' : 'play'} size={14} color="var(--color-ink)" />
        </button>
        <span class="flex min-w-0 flex-1 flex-col gap-1.5">
            <span bind:this={waveLane} class="relative flex h-11 items-center gap-[3px]">
                {#each displayHeights as height, barIndex (barIndex)}
                    {@const played = ready && (barIndex + 1) / displayHeights.length <= progress}
                    <span
                        class="shrink-0 rounded-full {played
                            ? 'bg-accent'
                            : ready
                              ? 'bg-ink-3 opacity-[0.55]'
                              : 'bg-surface-2'}"
                        style="width: 3px; height: {height}px;"
                    ></span>
                {/each}
                {#if ready}
                    <span
                        class="absolute top-0 bottom-0 w-[2px] rounded-full bg-accent shadow-[0_0_6px_var(--color-accent)]"
                        style="left: {progress * 100}%;"
                    ></span>
                    <input
                        class="absolute inset-0 h-full w-full cursor-pointer opacity-0 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                        type="range"
                        min="0"
                        max={Math.max(duration, 0.1)}
                        step="0.1"
                        value={position}
                        oninput={seekVoice}
                        aria-label="Seek voice message"
                    />
                {/if}
            </span>
            <span class="text-xs text-ink-3 tabular-nums">{ready ? timeLabel : statusLabel}</span>
        </span>
    </div>
{/if}
{#if sourceUrl !== ''}
    <audio
        bind:this={audioElement}
        class="sr-only"
        preload="metadata"
        src={sourceUrl}
        onplay={handlePlay}
        onpause={handlePause}
        onended={handlePause}
        ontimeupdate={syncFromElement}
        onloadedmetadata={syncFromElement}
    ></audio>
{/if}

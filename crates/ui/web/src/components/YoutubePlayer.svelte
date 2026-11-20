<script lang="ts">
    import type { AppState, WatchParty } from '../lib';
    import { formatPlayerTime } from '../lib';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
        party: WatchParty;
        selfId: string;
        onPublish: (positionMs: number, playing: boolean) => void;
    }

    let { app, party, selfId, onPublish }: Props = $props();
    let frame: HTMLDivElement | null = $state(null);
    let video: HTMLVideoElement | null = $state(null);
    let started = $state(false);
    let loading = $state(false);
    let buffering = $state(false);
    let loadFailed = $state(false);
    let streamUrl = $state('');
    let ready = $state(false);
    let pending = $state<WatchParty | null>(null);
    let position = $state(0);
    let duration = $state(0);
    let playing = $state(false);
    let muted = $state(false);
    let volume = $state(1);
    let lastRevision = -1;
    let lastActor = '';
    let requestToken = 0;

    const poster = $derived(`https://i.ytimg.com/vi/${party.video_id}/hqdefault.jpg`);
    const showPoster = $derived(
        !loadFailed && (!started || !ready || (!playing && position === 0)),
    );

    async function begin(auto = false): Promise<void> {
        if (loadFailed) {
            return;
        }
        if (!started) {
            started = true;
            loading = true;
            loadFailed = false;
            streamUrl = '';
            pending = auto ? party : { ...party, playing: true };
            const token = ++requestToken;
            const url = await app.youtubeStream(party.video_id);
            if (token !== requestToken) {
                return;
            }
            if (url === null) {
                loading = false;
                loadFailed = true;
                return;
            }
            streamUrl = url;
            if (!auto) {
                publish(position, true);
            }
            return;
        }
        if (video !== null && !playing) {
            const next = true;
            playing = next;
            const result = video.play();
            if (result !== undefined) {
                void result.catch(() => {});
            }
            publish(video.currentTime, next);
        }
    }

    function applyParty(value: WatchParty): void {
        if (video === null) {
            pending = value;
            return;
        }
        const seconds = Math.max(0, value.position_ms / 1_000);
        if (Math.abs(video.currentTime - seconds) > 0.5) {
            video.currentTime = seconds;
        }
        if (value.playing) {
            const result = video.play();
            if (result !== undefined) {
                void result.catch(() => {});
            }
        } else {
            video.pause();
        }
        position = seconds;
        playing = value.playing;
        lastRevision = value.revision;
        lastActor = value.actor;
    }

    function loaded(): void {
        if (video === null) {
            return;
        }
        ready = true;
        loading = false;
        buffering = false;
        loadFailed = false;
        duration = Number.isFinite(video.duration) ? video.duration : 0;
        applyAudio();
        const value = pending ?? party;
        pending = null;
        applyParty(value);
    }

    function applyAudio(): void {
        if (video === null) {
            return;
        }
        video.muted = muted;
        video.volume = volume;
    }

    function publish(nextPosition: number, nextPlaying: boolean): void {
        position = nextPosition;
        playing = nextPlaying;
        onPublish(Math.max(0, Math.round(nextPosition * 1_000)), nextPlaying);
    }

    function togglePlay(): void {
        if (video === null) {
            void begin();
            return;
        }
        const next = !playing;
        playing = next;
        if (next) {
            const result = video.play();
            if (result !== undefined) {
                void result.catch(() => {});
            }
        } else {
            video.pause();
        }
        publish(video.currentTime, next);
    }

    function seek(event: Event): void {
        if (video === null || !ready) {
            return;
        }
        const value = Number((event.currentTarget as HTMLInputElement).value);
        position = value;
        video.currentTime = value;
        publish(value, playing);
    }

    function toggleMute(): void {
        muted = !muted;
        applyAudio();
    }

    function changeVolume(event: Event): void {
        volume = Number((event.currentTarget as HTMLInputElement).value);
        applyAudio();
    }

    function fullscreen(): void {
        void frame?.requestFullscreen();
    }

    function retry(): void {
        requestToken += 1;
        started = false;
        ready = false;
        loading = false;
        buffering = false;
        loadFailed = false;
        streamUrl = '';
        duration = 0;
        position = 0;
        playing = false;
        void begin();
    }

    $effect(() => {
        const value = party;
        if (!started) {
            if (value.playing) {
                void begin(true);
            }
            return;
        }
        if (!ready) {
            pending = value;
            return;
        }
        if (value.revision === lastRevision && value.actor === lastActor) {
            return;
        }
        if (value.actor === selfId) {
            lastRevision = value.revision;
            lastActor = value.actor;
            return;
        }
        applyParty(value);
    });

    $effect(() => {
        if (party.role !== 'host' || !party.playing || !ready) {
            return;
        }
        const timer = window.setInterval(() => {
            if (video === null || video.paused) {
                return;
            }
            onPublish(Math.max(0, Math.round(video.currentTime * 1000)), true);
        }, 2_000);
        return () => window.clearInterval(timer);
    });
</script>

<div bind:this={frame} class="flex min-h-0 flex-1 flex-col">
    <div class="relative flex min-h-[300px] flex-1 overflow-hidden rounded-sm bg-base">
        {#if started && streamUrl !== ''}
            <video
                class="size-full bg-base object-contain"
                bind:this={video}
                src={streamUrl}
                playsinline
                preload="auto"
                onloadedmetadata={loaded}
                ontimeupdate={() => {
                    if (video !== null) {
                        position = video.currentTime;
                        duration = Number.isFinite(video.duration) ? video.duration : duration;
                        buffering = false;
                    }
                }}
                onplay={() => (playing = true)}
                onpause={() => (playing = false)}
                onwaiting={() => {
                    if (ready) {
                        buffering = true;
                    } else {
                        loading = true;
                    }
                }}
                onstalled={() => {
                    if (ready) {
                        buffering = true;
                    }
                }}
                oncanplay={() => (buffering = false)}
                onseeked={() => (buffering = false)}
                onplaying={() => {
                    loading = false;
                    buffering = false;
                }}
                onerror={() => {
                    if (streamUrl === '') {
                        return;
                    }
                    loading = false;
                    loadFailed = true;
                }}
            >
                <track kind="captions" src="data:text/vtt;charset=utf-8,WEBVTT%0A%0A" />
            </video>
        {/if}
        {#if showPoster}
            <button
                class="absolute inset-0 flex items-center justify-center overflow-hidden bg-base"
                onclick={() => void begin()}
                aria-label="Play video"
            >
                <img
                    class="absolute inset-0 h-full w-full object-cover opacity-80"
                    src={poster}
                    alt=""
                />
                <span
                    class="relative flex size-16 items-center justify-center rounded-pill bg-accent pl-0.5"
                >
                    <Icon name="play" size={24} color="var(--color-base)" />
                </span>
                {#if loading}
                    <span
                        class="absolute bottom-4 rounded-pill bg-base/80 px-3 py-1 text-xs text-ink"
                        >Starting video…</span
                    >
                {/if}
            </button>
        {/if}
        {#if buffering}
            <span
                class="pointer-events-none absolute bottom-4 left-1/2 -translate-x-1/2 rounded-pill bg-base/80 px-3 py-1 text-xs text-ink"
                >Buffering…</span
            >
        {/if}
        {#if loadFailed}
            <div
                class="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-surface-2 p-6 text-center"
            >
                <span class="text-step-xs text-ink-2">Video unavailable.</span>
                <button
                    class="inline-flex min-h-9 items-center justify-center rounded-pill border border-accent bg-accent px-4 text-step-xs font-bold text-base"
                    onclick={retry}
                >
                    Retry
                </button>
            </div>
        {/if}
    </div>
    <div class="flex items-center gap-2.5 px-1 pt-3 pb-1">
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
            onclick={togglePlay}
            aria-label={playing ? 'Pause' : 'Play'}
        >
            <Icon name={playing ? 'pause' : 'play'} size={16} />
        </button>
        <span class="w-14 text-right text-xs text-ink-3 tabular-nums"
            >{formatPlayerTime(position)}</span
        >
        <input
            class="flex-1 accent-accent disabled:opacity-50"
            type="range"
            min="0"
            max={Math.max(1, duration)}
            step="0.1"
            value={position}
            oninput={seek}
            disabled={!ready}
            aria-label="Seek video"
        />
        <span class="w-14 text-xs text-ink-3 tabular-nums">{formatPlayerTime(duration)}</span>
        <input
            class="h-8 w-20 shrink-0 accent-accent"
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={volume}
            oninput={changeVolume}
            aria-label="Volume"
        />
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
            onclick={toggleMute}
            aria-label={muted ? 'Unmute' : 'Mute'}
        >
            <Icon name={muted ? 'mute' : 'speaker'} size={15} />
        </button>
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
            onclick={fullscreen}
            aria-label="Fullscreen"
        >
            <Icon name="fullscreen" size={15} />
        </button>
    </div>
</div>

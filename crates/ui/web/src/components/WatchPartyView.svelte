<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState } from '../lib';
    import { sessionClock, shortName } from '../lib';
    import { firstVideoId } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let frame: HTMLIFrameElement | null = $state(null);
    let playing = $state(true);
    let position = $state(0);
    const party = $derived(app.party);
    const host = $derived(app.userById(party?.user_id ?? app.selectedUserId));
    const title = $derived(
        app.demo ? 'How we built TiConnect for the LAN' : `YouTube video ${party?.video_id ?? ''}`,
    );
    const participants = $derived(app.users.filter((user) => user.connected).slice(0, 3));
    const queue = $derived(
        app.demo
            ? ['demo-next-1', 'demo-next-2']
            : app.messages
                  .map((message) => firstVideoId(message.body))
                  .filter(
                      (videoId): videoId is string =>
                          videoId !== null && videoId !== party?.video_id,
                  )
                  .slice(0, 2),
    );

    function post(command: string, args: unknown[] = []): void {
        frame?.contentWindow?.postMessage(
            JSON.stringify({ event: 'command', func: command, args }),
            '*',
        );
    }

    function togglePlay(): void {
        playing = !playing;
        post(playing ? 'playVideo' : 'pauseVideo');
        if (party !== null) {
            void app.publishParty(position * 1_000, playing);
        }
    }

    function seek(event: Event): void {
        position = Number((event.currentTarget as HTMLInputElement).value);
        post('seekTo', [position, true]);
        if (party !== null) {
            void app.publishParty(position * 1_000, playing);
        }
    }

    function leave(): void {
        void app.endParty();
    }

    onMount(() => {
        if (app.party !== null) {
            playing = app.party.playing;
            position = Math.floor(app.party.position_ms / 1_000);
        }
        const timer = window.setInterval(() => {
            if (playing) {
                position += 1;
            }
        }, 1_000);
        return () => window.clearInterval(timer);
    });
</script>

<section class="flex h-full min-w-0 flex-1 gap-5 bg-base pt-[18px] pr-5 pb-5 pl-8">
    <button
        class="mt-1 inline-flex size-8 shrink-0 items-center justify-center self-start rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
        onclick={() => app.backToChat()}
        aria-label="Back"
    >
        <Icon name="back" size={16} />
    </button>
    {#if party}
        <div class="flex min-w-0 flex-1 gap-5">
            <div class="flex min-w-0 flex-1 flex-col">
                <div
                    class="flex min-h-0 flex-1 flex-col rounded-md border border-line bg-surface p-3"
                >
                    <div class="flex items-start justify-between gap-4 px-[2px] pt-0 pb-3">
                        <div>
                            <h1
                                class="m-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]"
                            >
                                {title}
                            </h1>
                            <p class="mt-[2px] mb-0 text-xs tracking-[0.06em] text-ink-3 uppercase">
                                TiConnect Labs
                            </p>
                        </div>
                        <span class="flex shrink-0 items-center gap-1.5 text-xs text-ink-3"
                            ><span class="size-1.5 rounded-full border border-success bg-success"
                            ></span>Synced · 0.2 s drift</span
                        >
                    </div>
                    <div class="flex min-h-[300px] flex-1 overflow-hidden rounded-sm bg-surface-2">
                        {#if app.demo}
                            <div
                                class="relative flex size-full items-center justify-center bg-surface-2"
                            >
                                <img
                                    class="size-full object-cover opacity-[0.45]"
                                    src={`https://i.ytimg.com/vi/${party.video_id}/mqdefault.jpg`}
                                    alt=""
                                />
                                <span
                                    class="absolute flex size-[72px] items-center justify-center rounded-full bg-accent"
                                    ><Icon name="play" size={22} color="var(--color-base)" /></span
                                >
                            </div>
                        {:else}
                            <iframe
                                class="size-full border-0"
                                bind:this={frame}
                                src={`https://www.youtube-nocookie.com/embed/${party.video_id}?enablejsapi=1&rel=0&autoplay=${playing ? 1 : 0}`}
                                {title}
                                allow="autoplay; encrypted-media; picture-in-picture"
                                allowfullscreen
                            ></iframe>
                        {/if}
                    </div>
                    <div class="flex items-center gap-2.5 pt-3">
                        <button
                            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                            onclick={togglePlay}
                            aria-label={playing ? 'Pause' : 'Play'}
                        >
                            <Icon name={playing ? 'pause' : 'play'} size={16} />
                        </button>
                        <span class="text-xs text-ink-3 tabular-nums">{sessionClock(position)}</span
                        >
                        <input
                            class="flex-1 accent-accent"
                            type="range"
                            min="0"
                            max="1122"
                            value={position}
                            oninput={seek}
                            aria-label="Seek video"
                        />
                        <span class="text-xs text-ink-3 tabular-nums">18:42</span>
                        <button
                            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                            aria-label="Mute"
                        >
                            <Icon name="speaker" size={15} />
                        </button>
                        <button
                            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                            aria-label="Fullscreen"
                        >
                            <Icon name="fullscreen" size={15} />
                        </button>
                    </div>
                </div>
            </div>
            <aside class="flex w-80 shrink-0 flex-col gap-4">
                <section class="rounded-md border border-line bg-surface p-4">
                    <h2
                        class="mt-0 mb-2.5 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Watching · {participants.length}
                    </h2>
                    {#each participants as user, index (user.id)}
                        <div class="flex items-center gap-2.5 border-b border-line py-2.5">
                            <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={32} />
                            <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                                <strong class="text-sm">{shortName(user)}</strong>
                                <span class="flex items-center gap-[5px] text-xs text-ink-3"
                                    ><span
                                        class="size-1.5 rounded-full border border-success bg-success"
                                    ></span>{index === 0 ? 'Started the party' : 'Watching'}</span
                                >
                            </div>
                            {#if index === 1}
                                <span
                                    class="rounded-pill bg-accent px-[9px] py-1 text-[11px] font-bold text-base"
                                    >Controlling</span
                                >
                            {/if}
                        </div>
                    {/each}
                    <p class="mt-3 mb-0 text-[13px] leading-[1.5] text-ink-3">
                        Anyone can play, pause, or seek. Changes apply to all.
                    </p>
                </section>
                <section class="rounded-md border border-line bg-surface p-4">
                    <h2
                        class="mt-0 mb-2.5 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Up next · {queue.length}
                    </h2>
                    {#each queue as videoId, index (videoId)}
                        <div
                            class="flex flex-col gap-0.5 border-b border-line py-[9px] last:border-b-0"
                        >
                            <strong class="text-[13px]"
                                >{index === 0
                                    ? 'LAN discovery in 90 seconds'
                                    : 'Building the overlay'}</strong
                            >
                            <span class="text-xs text-ink-3"
                                >TiConnect Labs · {index === 0 ? '3:08' : '7:21'}</span
                            >
                        </div>
                    {/each}
                    {#if queue.length === 0}
                        <p class="m-0 text-step-xs text-ink-3">Queue is quiet.</p>
                    {/if}
                </section>
                <button
                    class="mt-auto inline-flex min-h-9 w-full items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                    onclick={leave}>Leave party</button
                >
            </aside>
        </div>
    {:else}
        <div class="flex flex-1 flex-col items-center justify-center gap-2.5 text-center">
            <h1 class="m-0 font-display text-step-3 leading-[1.15] font-normal tracking-[-0.01em]">
                No watch party is active.
            </h1>
            <p class="m-0 text-step-xs text-ink-3">
                Open a conversation with a YouTube link to start one.
            </p>
            <button
                class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                onclick={() => app.backToChat()}>Back to chat</button
            >
        </div>
    {/if}
</section>

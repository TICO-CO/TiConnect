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

<section class="party">
    <button class="back icon-button small" onclick={() => app.backToChat()} aria-label="Back">
        <Icon name="back" size={16} />
    </button>
    {#if party}
        <div class="party-body">
            <div class="stage-column">
                <div class="stage">
                    <div class="stage-head">
                        <div>
                            <h1 class="display">{title}</h1>
                            <p>TiConnect Labs</p>
                        </div>
                        <span class="synced"><span class="dot"></span>Synced · 0.2 s drift</span>
                    </div>
                    <div class="video">
                        {#if app.demo}
                            <div class="mock-video">
                                <img
                                    src={`https://i.ytimg.com/vi/${party.video_id}/mqdefault.jpg`}
                                    alt=""
                                />
                                <span class="mock-play"
                                    ><Icon name="play" size={22} color="var(--base)" /></span
                                >
                            </div>
                        {:else}
                            <iframe
                                bind:this={frame}
                                src={`https://www.youtube-nocookie.com/embed/${party.video_id}?enablejsapi=1&rel=0&autoplay=${playing ? 1 : 0}`}
                                {title}
                                allow="autoplay; encrypted-media; picture-in-picture"
                                allowfullscreen
                            ></iframe>
                        {/if}
                    </div>
                    <div class="player-controls">
                        <button
                            class="icon-button small"
                            onclick={togglePlay}
                            aria-label={playing ? 'Pause' : 'Play'}
                        >
                            <Icon name={playing ? 'pause' : 'play'} size={16} />
                        </button>
                        <span class="time mono-data">{sessionClock(position)}</span>
                        <input
                            class="seek"
                            type="range"
                            min="0"
                            max="1122"
                            value={position}
                            oninput={seek}
                            aria-label="Seek video"
                        />
                        <span class="time mono-data">18:42</span>
                        <button class="icon-button small" aria-label="Mute">
                            <Icon name="speaker" size={15} />
                        </button>
                        <button class="icon-button small" aria-label="Fullscreen">
                            <Icon name="fullscreen" size={15} />
                        </button>
                    </div>
                </div>
            </div>
            <aside class="side">
                <section class="side-card">
                    <h2 class="label">Watching · {participants.length}</h2>
                    {#each participants as user, index (user.id)}
                        <div class="participant">
                            <Avatar {user} size={32} />
                            <div class="participant-copy">
                                <strong>{shortName(user)}</strong>
                                <span
                                    ><span class="dot online"></span>{index === 0
                                        ? 'Started the party'
                                        : 'Watching'}</span
                                >
                            </div>
                            {#if index === 1}
                                <span class="controller">Controlling</span>
                            {/if}
                        </div>
                    {/each}
                    <p class="side-note">Anyone can play, pause, or seek. Changes apply to all.</p>
                </section>
                <section class="side-card">
                    <h2 class="label">Up next · {queue.length}</h2>
                    {#each queue as videoId, index (videoId)}
                        <div class="queue-item">
                            <strong
                                >{index === 0
                                    ? 'LAN discovery in 90 seconds'
                                    : 'Building the overlay'}</strong
                            >
                            <span>TiConnect Labs · {index === 0 ? '3:08' : '7:21'}</span>
                        </div>
                    {/each}
                    {#if queue.length === 0}
                        <p class="empty">Queue is quiet.</p>
                    {/if}
                </section>
                <button class="btn btn-danger leave" onclick={leave}>Leave party</button>
            </aside>
        </div>
    {:else}
        <div class="waiting">
            <h1 class="display">No watch party is active.</h1>
            <p class="empty">Open a conversation with a YouTube link to start one.</p>
            <button class="btn btn-accent" onclick={() => app.backToChat()}>Back to chat</button>
        </div>
    {/if}
</section>

<style>
    .party {
        display: flex;
        height: 100%;
        flex: 1;
        min-width: 0;
        padding: 18px 20px 20px 32px;
        gap: 20px;
        background: var(--base);
    }

    .back {
        align-self: flex-start;
        margin-top: 4px;
    }

    .party-body {
        display: flex;
        flex: 1;
        min-width: 0;
        gap: 20px;
    }

    .stage-column {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
    }

    .stage {
        display: flex;
        flex-direction: column;
        min-height: 0;
        flex: 1;
        padding: 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .stage-head {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 16px;
        padding: 0 2px 12px;
    }

    .stage-head h1 {
        margin: 0;
        font-size: 19px;
        line-height: 24px;
    }

    .stage-head p {
        margin: 2px 0 0;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .synced {
        display: flex;
        align-items: center;
        gap: 6px;
        flex-shrink: 0;
        color: var(--ink-3);
        font-size: 12px;
    }

    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        border: 1px solid var(--line-strong);
    }

    .dot.online,
    .synced .dot {
        background: var(--success);
        border-color: var(--success);
    }

    .video {
        display: flex;
        min-height: 300px;
        flex: 1;
        overflow: hidden;
        border-radius: var(--radius-s);
        background: var(--surface-2);
    }

    .video iframe {
        width: 100%;
        height: 100%;
        border: 0;
    }

    .mock-video {
        position: relative;
        display: flex;
        width: 100%;
        height: 100%;
        align-items: center;
        justify-content: center;
        background: var(--surface-2);
    }

    .mock-video img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        opacity: 0.45;
    }

    .mock-play {
        position: absolute;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 72px;
        height: 72px;
        border-radius: 50%;
        background: var(--accent);
    }

    .player-controls {
        display: flex;
        align-items: center;
        gap: 10px;
        padding-top: 12px;
    }

    .seek {
        flex: 1;
        accent-color: var(--accent);
    }

    .time {
        color: var(--ink-3);
        font-size: 12px;
    }

    .side {
        display: flex;
        flex-direction: column;
        width: 320px;
        flex-shrink: 0;
        gap: 16px;
    }

    .side-card {
        padding: 16px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .side-card h2 {
        margin: 0 0 10px;
    }

    .participant {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 0;
        border-bottom: 1px solid var(--line);
    }

    .participant-copy {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        flex: 1;
    }

    .participant-copy strong {
        font-size: 14px;
    }

    .participant-copy span {
        display: flex;
        align-items: center;
        gap: 5px;
        color: var(--ink-3);
        font-size: 12px;
    }

    .controller {
        padding: 4px 9px;
        border-radius: var(--radius-pill);
        background: var(--accent);
        color: var(--base);
        font-size: 11px;
        font-weight: 700;
    }

    .side-note {
        margin: 12px 0 0;
        color: var(--ink-3);
        font-size: 13px;
        line-height: 1.5;
    }

    .queue-item {
        display: flex;
        flex-direction: column;
        gap: 2px;
        padding: 9px 0;
        border-bottom: 1px solid var(--line);
    }

    .queue-item:last-child {
        border-bottom: 0;
    }

    .queue-item strong {
        font-size: 13px;
    }

    .queue-item span {
        color: var(--ink-3);
        font-size: 12px;
    }

    .leave {
        width: 100%;
        margin-top: auto;
    }

    .waiting {
        display: flex;
        flex: 1;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 10px;
        text-align: center;
    }

    .waiting h1 {
        margin: 0;
        font-size: var(--step-3);
    }
</style>

<script lang="ts">
    import type { AppState } from '../lib';
    import { clock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let playerFrame: HTMLIFrameElement | null = $state(null);
    let muted = $state(false);
    let now = $state(Date.now());

    $effect(() => {
        const timer = window.setInterval(() => {
            now = Date.now();
        }, 1_000);
        return () => window.clearInterval(timer);
    });

    const incomingCall = $derived(
        app.call !== null && app.call.state === 'ringing' && app.call.direction === 'incoming'
            ? app.call
            : null,
    );
    const activeCall = $derived(
        app.call !== null && app.call.state !== 'ringing' && app.route.kind !== 'call'
            ? app.call
            : null,
    );
    const remoteRequest = $derived(
        app.remote !== null && app.remote.state === 'requested' && app.remote.role === 'host'
            ? app.remote
            : null,
    );
    const activeRemote = $derived(
        app.remote !== null && app.remote.state === 'active' && app.route.kind !== 'remote'
            ? app.remote
            : null,
    );
    const partyInvite = $derived(
        app.party !== null && app.party.state === 'ringing' ? app.party : null,
    );
    const activeParty = $derived(
        app.party !== null && app.party.state !== 'ringing' && app.route.kind !== 'watch-party'
            ? app.party
            : null,
    );
    const callSeconds = $derived(
        app.call === null
            ? 0
            : Math.max(
                  0,
                  Math.floor((now - (app.call.connected_at || app.call.started_at)) / 1_000),
              ),
    );
    const remoteSeconds = $derived(
        app.remote === null
            ? 0
            : Math.max(0, 300 - Math.floor((app.remote.expires_at - now) / 1_000)),
    );

    function post(command: string): void {
        playerFrame?.contentWindow?.postMessage(
            JSON.stringify({ event: 'command', func: command, args: [] }),
            '*',
        );
    }

    function openToast(userId: string, toastId: string): void {
        app.dismissToast(toastId);
        app.navigate({ kind: 'chat', id: userId });
    }

    function partyTitle(): string {
        return app.demo ? 'How we built TiConnect for the LAN' : 'YouTube watch party';
    }
</script>

<div class="request-stack" aria-live="polite">
    {#if incomingCall}
        {@const caller = app.userById(incomingCall.user_id)}
        <article class="request-card">
            <div class="card-head">
                <Avatar
                    user={caller}
                    image={caller ? app.avatarUrl(caller.fingerprint) : ''}
                    name={app.userName(incomingCall.user_id)}
                    size={32}
                />
                <span class="label">Incoming call</span>
                <span class="time mono-data">{clock(incomingCall.started_at)}</span>
            </div>
            <h2 class="display">{app.userName(incomingCall.user_id)}</h2>
            <p class="card-sub">Voice call · LAN</p>
            <div class="card-actions">
                <button class="btn btn-small btn-danger" onclick={() => void app.declineCall()}
                    >Decline</button
                >
                <button class="btn btn-small btn-accent" onclick={() => void app.acceptCall()}
                    >Accept</button
                >
            </div>
        </article>
    {/if}
    {#if remoteRequest}
        {@const requester = app.userById(remoteRequest.user_id)}
        <article class="request-card">
            <div class="card-head">
                <Avatar
                    user={requester}
                    image={requester ? app.avatarUrl(requester.fingerprint) : ''}
                    name={app.userName(remoteRequest.user_id)}
                    size={32}
                />
                <span class="label">Remote desktop</span>
                <span class="time mono-data">Now</span>
            </div>
            <h2 class="display">{app.userName(remoteRequest.user_id)}</h2>
            <p class="card-sub">Wants to view and control this screen</p>
            <div class="card-actions">
                <button class="btn btn-small btn-danger" onclick={() => void app.denyRemote()}
                    >Decline</button
                >
                <button class="btn btn-small btn-accent" onclick={() => void app.acceptRemote()}
                    >Accept</button
                >
            </div>
        </article>
    {/if}
    {#if partyInvite}
        {@const partyUser = app.userById(partyInvite.user_id)}
        <article class="request-card">
            <div class="card-head">
                <Avatar
                    user={partyUser}
                    image={partyUser ? app.avatarUrl(partyUser.fingerprint) : ''}
                    name={app.userName(partyInvite.user_id)}
                    size={32}
                />
                <span class="label">Watch party invite</span>
                <span class="time mono-data">{clock(partyInvite.updated_at)}</span>
            </div>
            <h2 class="display">{app.userName(partyInvite.user_id)}</h2>
            <p class="card-sub">Watch party · {partyTitle()}</p>
            <div class="card-actions">
                <button class="btn btn-small btn-danger" onclick={() => void app.declineParty()}
                    >Decline</button
                >
                <button class="btn btn-small btn-accent" onclick={() => void app.acceptParty()}
                    >Join</button
                >
            </div>
        </article>
    {/if}
    {#if activeCall}
        {@const callUser = app.userById(activeCall.user_id)}
        <article class="request-card active">
            <div class="card-head">
                <span class="status-dot"></span>
                <span class="label">In call</span>
                <span class="time mono-data"
                    >{new Date(callSeconds * 1000).toISOString().slice(11, 19)}</span
                >
            </div>
            <h2 class="display">{app.userName(activeCall.user_id)}</h2>
            <div class="profile-ident">
                <Avatar
                    user={callUser}
                    image={callUser ? app.avatarUrl(callUser.fingerprint) : ''}
                    name={app.userName(activeCall.user_id)}
                    size={24}
                />
                <span>{app.userName(activeCall.user_id)}</span>
            </div>
            <div class="card-actions">
                <button class="btn btn-small">Mute <Icon name="mute" size={12} /></button>
                <button class="btn btn-small btn-danger" onclick={() => void app.hangupCall()}
                    >End</button
                >
            </div>
        </article>
    {/if}
    {#if activeRemote}
        <article class="request-card active">
            <div class="card-head">
                <span class="status-dot"></span>
                <span class="label">Sharing screen</span>
                <span class="time mono-data"
                    >{new Date(remoteSeconds * 1000).toISOString().slice(11, 19)}</span
                >
            </div>
            <h2 class="display">{app.userName(activeRemote.user_id)}</h2>
            <p class="card-sub">Indicator stays visible while sharing.</p>
            <div class="card-actions">
                <button class="btn btn-small btn-danger" onclick={() => void app.endRemote()}
                    >End session</button
                >
            </div>
        </article>
    {/if}
    {#if activeParty}
        <button class="party-pill" onclick={() => app.navigate({ kind: 'watch-party' })}>
            <span class="pill-icon"><Icon name="play" size={12} color="var(--ink-2)" /></span>
            <span>Watch party · 3 watching</span>
            <span class="open-pill">Open</span>
        </button>
    {/if}
</div>

<div
    class="toast-stack"
    style={`bottom:${app.player && app.route.kind !== 'watch-party' ? 442 : 24}px`}
    aria-live="polite"
>
    {#each app.toasts as toast (toast.id)}
        <div
            class="toast"
            role="button"
            tabindex="0"
            onclick={() => openToast(toast.userId, toast.id)}
            onkeydown={(event) => {
                if (event.key === 'Enter') {
                    openToast(toast.userId, toast.id);
                }
            }}
        >
            <Avatar name={toast.name} size={32} />
            <span class="toast-copy">
                <span class="toast-head">
                    <span class="toast-name">{toast.name}</span>
                    <span class="time mono-data">{clock(toast.sentAt)}</span>
                </span>
                <span class="toast-body">{toast.body}</span>
            </span>
            <button
                class="toast-dismiss"
                onclick={(event) => {
                    event.stopPropagation();
                    app.dismissToast(toast.id);
                }}
                aria-label="Dismiss"
            >
                <Icon name="close" size={13} color="var(--ink-3)" />
            </button>
        </div>
    {/each}
</div>

{#if app.player && app.route.kind !== 'watch-party'}
    <div class="player">
        {#if app.messages.length > 0}
            <div class="up-next">
                <span class="label">Up next · 2</span>
                <strong>LAN discovery in 90 seconds</strong>
            </div>
        {/if}
        <div class="player-card">
            <div class="player-head">
                <span class="label">YouTube</span>
                <span class="ignore">
                    Ignore incoming
                    <button
                        class="toggle small"
                        aria-label="Ignore incoming videos"
                        aria-pressed={app.preferences.ignoreIncoming}
                        onclick={() => app.toggleIgnoreIncoming()}
                    ></button>
                </span>
                <button
                    class="icon-button small"
                    onclick={() => app.closePlayer()}
                    aria-label="Close player"
                >
                    <Icon name="close" size={14} />
                </button>
            </div>
            <div class="player-video">
                <iframe
                    bind:this={playerFrame}
                    src={`https://www.youtube-nocookie.com/embed/${app.player.videoId}?enablejsapi=1&rel=0&autoplay=1`}
                    title={app.player.title}
                    allow="autoplay; encrypted-media; picture-in-picture"
                    allowfullscreen
                ></iframe>
            </div>
            <div class="player-copy">
                <strong>{app.player.title}</strong>
                <span>{app.player.channel} · 18:42</span>
            </div>
            <div class="player-controls">
                <button
                    class="icon-button small"
                    onclick={() => post('playVideo')}
                    aria-label="Play"
                >
                    <Icon name="play" size={14} />
                </button>
                <button
                    class="icon-button small"
                    onclick={() => post('pauseVideo')}
                    aria-label="Pause"
                >
                    <Icon name="pause" size={14} />
                </button>
                <span class="time mono-data">06:12</span>
                <span class="progress"><span></span></span>
                <span class="time mono-data">18:42</span>
                <button
                    class="icon-button small"
                    onclick={() => {
                        muted = !muted;
                        post(muted ? 'mute' : 'unMute');
                    }}
                    aria-label={muted ? 'Unmute' : 'Mute'}
                >
                    <Icon name={muted ? 'mute' : 'speaker'} size={14} />
                </button>
                <button class="icon-button small" aria-label="Fullscreen">
                    <Icon name="fullscreen" size={14} />
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .request-stack,
    .toast-stack {
        position: fixed;
        right: 24px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        z-index: 30;
        pointer-events: none;
    }

    .request-stack {
        top: 24px;
        width: 360px;
    }

    .toast-stack {
        width: 420px;
        transition: bottom 200ms var(--ease-soft);
    }

    .request-card,
    .toast,
    .party-pill,
    .player {
        pointer-events: auto;
    }

    .request-card {
        display: flex;
        flex-direction: column;
        padding: 14px 16px;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-l);
        background: var(--surface);
        box-shadow: var(--shadow-soft);
    }

    .card-head {
        display: flex;
        align-items: center;
        gap: 8px;
        min-height: 32px;
    }

    .card-head .label {
        flex: 1;
    }

    .time {
        color: var(--ink-3);
        font-size: 12px;
    }

    .request-card h2 {
        margin: 8px 0 0;
        font-size: 19px;
        line-height: 24px;
    }

    .card-sub {
        margin: 2px 0 0;
        color: var(--ink-2);
        font-size: 13px;
    }

    .card-actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
        margin-top: 12px;
    }

    .status-dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        background: var(--accent);
    }

    .active .status-dot {
        background: var(--success);
    }

    .profile-ident {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-top: 8px;
        color: var(--ink-3);
        font-size: 12px;
    }

    .party-pill {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
        height: 46px;
        padding-inline: 14px 8px;
        border: 1px solid var(--line);
        border-radius: var(--radius-pill);
        background: var(--surface-2);
        color: var(--ink);
        box-shadow: var(--shadow-soft);
        font-size: 13px;
    }

    .pill-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 30px;
        height: 30px;
        border: 1px solid var(--line-strong);
        border-radius: 50%;
    }

    .open-pill {
        margin-left: auto;
        padding: 4px 10px;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-pill);
        color: var(--ink-2);
        font-size: 12px;
    }

    .toast {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
        padding: 12px 14px;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-l);
        background: var(--surface-2);
        color: var(--ink);
        box-shadow: var(--shadow-soft);
        text-align: left;
    }

    .toast-copy {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        flex: 1;
    }

    .toast-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
    }

    .toast-name {
        color: var(--ink);
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .toast-body {
        overflow: hidden;
        color: var(--ink);
        font-size: 14px;
        line-height: 1.4;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .toast-dismiss {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 24px;
        height: 24px;
        flex-shrink: 0;
        border-radius: 50%;
    }

    .toast-dismiss:hover {
        background: var(--surface);
    }

    .player {
        position: fixed;
        right: 24px;
        bottom: 24px;
        width: 480px;
        z-index: 31;
    }

    .up-next {
        display: flex;
        flex-direction: column;
        gap: 2px;
        margin-bottom: 8px;
        padding: 8px 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface-2);
    }

    .up-next strong {
        font-size: 12px;
    }

    .player-card {
        display: flex;
        flex-direction: column;
        padding: 12px;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-l);
        background: var(--surface);
        box-shadow: var(--shadow-soft);
    }

    .player-head {
        display: flex;
        align-items: center;
        gap: 8px;
        padding-bottom: 10px;
    }

    .player-head .label {
        color: var(--ink-2);
        flex: 1;
    }

    .ignore {
        display: flex;
        align-items: center;
        gap: 8px;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .toggle.small {
        width: 34px;
        height: 18px;
    }

    .toggle.small::after {
        width: 14px;
        height: 14px;
    }

    .toggle.small[aria-pressed='true']::after {
        transform: translateX(16px);
    }

    .player-video {
        display: flex;
        aspect-ratio: 16 / 9;
        overflow: hidden;
        border-radius: var(--radius-s);
        background: var(--surface-2);
    }

    .player-video iframe {
        width: 100%;
        height: 100%;
        border: 0;
    }

    .player-copy {
        display: flex;
        flex-direction: column;
        gap: 2px;
        padding: 10px 0;
    }

    .player-copy strong {
        font-size: 15px;
    }

    .player-copy span {
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .player-controls {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .progress {
        flex: 1;
        height: 3px;
        border-radius: var(--radius-pill);
        background: var(--line);
    }

    .progress span {
        display: block;
        width: 34%;
        height: 100%;
        border-radius: var(--radius-pill);
        background: var(--accent);
    }
</style>

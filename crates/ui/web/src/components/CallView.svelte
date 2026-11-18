<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState } from '../lib';
    import { initials, sessionClock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let muted = $state(false);
    let outputMuted = $state(false);
    let elapsed = $state(0);
    const user = $derived(app.userById(app.call?.user_id ?? app.selectedUserId));
    const active = $derived(app.call);
    const ringing = $derived(active?.state === 'ringing');
    const reconnecting = $derived(active?.state === 'reconnecting');

    onMount(() => {
        const update = () => {
            if (app.call === null) {
                elapsed = 0;
                return;
            }
            const start = app.call.connected_at || app.call.started_at;
            elapsed = Math.max(0, Math.floor((Date.now() - start) / 1_000));
        };
        update();
        const timer = window.setInterval(update, 1_000);
        return () => window.clearInterval(timer);
    });
</script>

<section class="call-pane">
    <header class="call-head">
        {#if user}
            <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={40} />
            <div class="head-copy">
                <h1 class="display">{shortName(user)}</h1>
                <p>
                    <span class="dot" class:online={user.connected}></span>
                    Voice call · {ringing ? 'Ringing' : reconnecting ? 'Reconnecting' : 'Encrypted'}
                </p>
            </div>
        {/if}
        <div class="live">
            <strong class="mono-data">{sessionClock(elapsed)}</strong>
            <span class="label">Live · LAN</span>
        </div>
    </header>
    {#if active && user}
        <div class="stage">
            <div class="rings" class:reconnecting>
                <span class="ring ring-1"></span>
                <span class="ring ring-2"></span>
                <span class="monogram">{initials(shortName(user))}</span>
            </div>
            <h2 class="display">{shortName(user)}</h2>
            <p class="timer mono-data">{sessionClock(elapsed)}</p>
            <p class="status">
                <span class="dot" class:warning={reconnecting} class:online={!reconnecting}></span>
                {ringing
                    ? 'Calling…'
                    : reconnecting
                      ? 'Reconnecting — audio resumes automatically'
                      : 'Connected · Opus · LAN'}
            </p>
            {#if muted}
                <span class="muted-pill"
                    ><Icon name="mute" size={13} color="var(--base)" /> Muted</span
                >
            {/if}
        </div>
        <div class="controls">
            <div class="control-card">
                <button class="control" class:active={muted} onclick={() => (muted = !muted)}>
                    <span class="control-icon"
                        ><Icon name={muted ? 'mute' : 'mic'} size={18} /></span
                    >
                    <span>{muted ? 'Unmute' : 'Mute'}</span>
                </button>
                <button
                    class="control"
                    class:active={outputMuted}
                    onclick={() => (outputMuted = !outputMuted)}
                >
                    <span class="control-icon"
                        ><Icon name={outputMuted ? 'mute' : 'speaker'} size={18} /></span
                    >
                    <span>Output</span>
                </button>
                <button class="control end" onclick={() => void app.hangupCall()}>
                    <span class="control-icon"
                        ><Icon name="call" size={18} color="var(--error)" /></span
                    >
                    <span>End call</span>
                </button>
            </div>
        </div>
    {:else}
        <div class="ended">
            <div class="ended-line"><span></span>Call ended<span></span></div>
            <button
                class="btn btn-accent"
                onclick={() => (user ? void app.startCall(user.id) : undefined)}>Call back</button
            >
        </div>
    {/if}
</section>

<style>
    .call-pane {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
        height: 100%;
        background: var(--base);
    }

    .call-head {
        display: flex;
        align-items: center;
        height: 88px;
        flex-shrink: 0;
        padding-inline: 28px;
        gap: 14px;
        background: var(--rail);
    }

    .head-copy {
        display: flex;
        flex-direction: column;
        gap: 3px;
        min-width: 0;
        flex: 1;
    }

    .head-copy h1 {
        margin: 0;
        font-size: var(--step-2);
        line-height: 28px;
    }

    .head-copy p {
        display: flex;
        align-items: center;
        gap: 6px;
        margin: 0;
        color: var(--ink-3);
        font-size: 13px;
    }

    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        border: 1px solid var(--line-strong);
    }

    .dot.online {
        background: var(--success);
        border-color: var(--success);
    }

    .dot.warning {
        background: var(--warning);
        border-color: var(--warning);
    }

    .live {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 2px;
        margin-left: auto;
    }

    .live strong {
        font-size: 23px;
        line-height: 26px;
        font-weight: 400;
    }

    .stage {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex: 1;
        gap: 8px;
    }

    .rings {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
        width: 300px;
        height: 300px;
    }

    .ring {
        position: absolute;
        border: 1px solid color-mix(in srgb, var(--line-strong) 80%, transparent);
        border-radius: 50%;
    }

    .ring-1 {
        width: 300px;
        height: 300px;
    }

    .ring-2 {
        width: 238px;
        height: 238px;
    }

    .rings.reconnecting .ring {
        border-color: color-mix(in srgb, var(--warning) 55%, transparent);
    }

    .monogram {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 176px;
        height: 176px;
        border-radius: 50%;
        background: var(--surface);
        color: var(--ink-2);
        font-size: 47px;
    }

    .stage h2 {
        margin: 1px 0 0;
        font-size: 39px;
        line-height: 44px;
    }

    .timer {
        margin: 0;
        color: var(--ink-2);
        font-size: 33px;
        font-weight: 400;
        line-height: 38px;
        font-variant-numeric: tabular-nums;
    }

    .status {
        display: flex;
        align-items: center;
        gap: 6px;
        margin: 0;
        color: var(--ink-3);
        font-size: 13px;
    }

    .muted-pill {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        margin-top: 4px;
        padding: 5px 10px;
        border-radius: var(--radius-pill);
        background: var(--accent);
        color: var(--base);
        font-size: 12px;
        font-weight: 700;
    }

    .controls {
        display: flex;
        justify-content: center;
        flex-shrink: 0;
        padding-bottom: 36px;
    }

    .control-card {
        display: flex;
        align-items: stretch;
        gap: 16px;
        padding: 14px 18px;
        border: 1px solid var(--line);
        border-radius: var(--radius-l);
        background: var(--surface);
    }

    .control {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 6px;
        min-width: 64px;
        border: 0;
        background: transparent;
        color: var(--ink-3);
        font-size: 12px;
    }

    .control-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 38px;
        height: 38px;
        border-radius: 50%;
        border: 1px solid var(--line-strong);
    }

    .control:hover .control-icon {
        background: var(--surface-2);
    }

    .control.active {
        color: var(--accent);
    }

    .control.active .control-icon {
        border-color: var(--accent);
    }

    .control.end {
        color: var(--error);
    }

    .control.end .control-icon {
        border-color: var(--error);
    }

    .ended {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex: 1;
        gap: 18px;
    }

    .ended-line {
        display: flex;
        align-items: center;
        justify-content: center;
        width: min(560px, 70%);
        gap: 12px;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .ended-line span {
        height: 1px;
        flex: 1;
        background: var(--line);
    }
</style>

<script lang="ts">
    import type { AppState } from '../lib';
    import { sessionClock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let inputAllowed = $state(true);
    const session = $derived(app.remote);
    const user = $derived(app.userById(session?.user_id ?? app.selectedUserId));
    const active = $derived(session?.state === 'active');
    const waiting = $derived(session === null || session.state === 'requested');
    const host = $derived(session?.role === 'host');
    const elapsed = $derived(
        app.demo ? 221 : Math.max(0, (Date.now() - (session?.expires_at ?? 0)) / -1000),
    );
</script>

<section class="remote-pane">
    <header class="remote-head">
        <button class="icon-button small" onclick={() => app.backToChat()} aria-label="Back">
            <Icon name="back" size={16} />
        </button>
        <div class="head-copy">
            <h1 class="display">
                {#if user}
                    {host ? `Sharing with ${shortName(user)}` : `${shortName(user)} is sharing`}
                {:else}
                    Remote desktop
                {/if}
            </h1>
            <p>Monitor 1 · Encrypted · LAN</p>
        </div>
        <span class="live-pill"
            ><span class="dot"></span>Live · {sessionClock(Math.max(0, elapsed))}</span
        >
    </header>
    {#if waiting}
        <div class="waiting">
            <span class="wait-monogram"><Icon name="remote" size={28} color="var(--ink-2)" /></span>
            <h2 class="display">{host ? 'Waiting for the request' : 'Waiting for consent'}</h2>
            <p class="empty">
                {host
                    ? 'The request stays open until you accept or it expires.'
                    : 'Capture starts only after the other side accepts.'}
            </p>
            <div class="wait-actions">
                <button
                    class="btn btn-danger"
                    onclick={() => (host ? void app.denyRemote() : void app.endRemote())}
                >
                    Cancel request
                </button>
                {#if host && session}
                    <button class="btn btn-accent" onclick={() => void app.acceptRemote()}
                        >Accept</button
                    >
                {/if}
            </div>
        </div>
    {:else}
        <div class="remote-body">
            <div class="viewer-column">
                <div class="stage">
                    <div class="stage-bar">
                        <span class="share-label"
                            ><span class="lamp"></span>Sharing screen · {user
                                ? shortName(user)
                                : ''}</span
                        >
                        <span class="quality">LAN · 1080p · 24 fps</span>
                    </div>
                    <div class="mock-screen">
                        <div class="mock-app">
                            <div class="mock-top">
                                <span></span><span></span><span></span>
                                <span class="mock-file">ticonnect — release-notes.md</span>
                            </div>
                            <div class="mock-columns">
                                <div class="mock-side">
                                    <span class="label">Explorer</span>
                                    <strong>TICONNECT</strong>
                                    <span>crates/</span><span>docs/</span><span
                                        class="mock-selected">release-notes.md</span
                                    >
                                    <span>ux-review.md</span><span>CHANGELOG.md</span>
                                </div>
                                <div class="mock-main">
                                    <h3 class="display">TiConnect 0.9.2</h3>
                                    <p>Quiet Grove release · LAN-only</p>
                                    <ul>
                                        <li>Messages, calls, and sharing stay on the LAN</li>
                                        <li>No accounts, no relays, no cloud storage</li>
                                        <li>Remote desktop requires fresh host consent</li>
                                        <li>48 tests passing · 0 warnings</li>
                                    </ul>
                                </div>
                                <div class="mock-meta">
                                    <span class="label">Build</span><strong class="success"
                                        >Passing</strong
                                    >
                                    <span>cargo test · workspace</span>
                                    <span class="label">Branch</span><strong>main</strong>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <p class="monitor-note">
                    <span class="lamp"></span>Monitor 1 · 2560 × 1440 · indicator stays visible
                    while sharing
                </p>
                {#if host}
                    <section class="session-log">
                        <h2 class="label">This session</h2>
                        <div>
                            <span>09:38</span><span>Sharing started</span><span>Monitor 1</span>
                        </div>
                        <div>
                            <span>09:40</span><span>{user ? shortName(user) : 'Guest'} joined</span
                            ><span>Encrypted</span>
                        </div>
                        <div>
                            <span>09:41</span><span>Keyboard control enabled</span><span
                                >{inputAllowed ? 'Allowed' : 'Blocked'}</span
                            >
                        </div>
                    </section>
                {/if}
            </div>
            <aside class="side">
                {#if host}
                    <section class="side-card">
                        <div class="side-head">
                            <h2 class="label">Sharing</h2>
                            <span>{sessionClock(Math.max(0, elapsed))}</span>
                        </div>
                        <p class="share-title"><span class="lamp"></span>Sharing screen</p>
                        <div class="peer-row">
                            {#if user}<Avatar
                                    {user}
                                    image={app.avatarUrl(user.fingerprint)}
                                    size={32}
                                />{/if}
                            <div>
                                <strong>{user ? shortName(user) : 'Remote viewer'}</strong><span
                                    >Encrypted · LAN</span
                                >
                            </div>
                        </div>
                        <div class="option-row">
                            <div>
                                <strong>Allow pointer and keyboard</strong><span
                                    >Control stops when sharing ends</span
                                >
                            </div>
                            <button
                                class="toggle"
                                aria-label="Allow pointer and keyboard"
                                aria-pressed={inputAllowed}
                                onclick={() => (inputAllowed = !inputAllowed)}
                            ></button>
                        </div>
                        <p class="side-note">
                            Stop sharing ends capture and input immediately. The indicator
                            disappears only after teardown.
                        </p>
                        <button class="btn btn-danger wide" onclick={() => void app.endRemote()}
                            >Stop sharing</button
                        >
                    </section>
                {:else}
                    <section class="side-card">
                        <h2 class="label">Session</h2>
                        <div class="peer-row">
                            {#if user}<Avatar
                                    {user}
                                    image={app.avatarUrl(user.fingerprint)}
                                    size={32}
                                />{/if}
                            <div>
                                <strong>{user ? shortName(user) : 'Host'}</strong><span
                                    >Encrypted · LAN</span
                                >
                            </div>
                        </div>
                        <p class="side-note">
                            You are viewing this screen. Input is controlled by the host.
                        </p>
                        <button class="btn btn-danger wide" onclick={() => void app.endRemote()}
                            >Leave session</button
                        >
                    </section>
                {/if}
            </aside>
        </div>
        {#if !host}
            <div class="viewer-controls">
                <button class="btn btn-small"><Icon name="pointer" size={14} />Pointer</button>
                <button class="btn btn-small"><Icon name="keyboard" size={14} />Keyboard</button>
                <button class="btn btn-small"><Icon name="fullscreen" size={14} />Fullscreen</button
                >
                <button class="btn btn-small btn-danger" onclick={() => void app.endRemote()}
                    >End session</button
                >
            </div>
        {/if}
    {/if}
</section>

<style>
    .remote-pane {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
        height: 100%;
        background: var(--base);
        overflow: hidden;
    }

    .remote-head {
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
        gap: 2px;
        min-width: 0;
        flex: 1;
    }

    .head-copy h1 {
        margin: 0;
        font-size: var(--step-3);
        line-height: 34px;
    }

    .head-copy p {
        margin: 0;
        color: var(--ink-3);
        font-size: 13px;
    }

    .live-pill {
        display: flex;
        align-items: center;
        gap: 6px;
        height: 34px;
        padding-inline: 14px;
        border: 1px solid var(--line);
        border-radius: var(--radius-pill);
        color: var(--ink-2);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .dot,
    .lamp {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        background: var(--error);
    }

    .lamp {
        display: inline-block;
        margin-right: 6px;
    }

    .waiting {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex: 1;
        gap: 10px;
        text-align: center;
    }

    .wait-monogram {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 88px;
        height: 88px;
        border-radius: 50%;
        background: var(--surface);
    }

    .waiting h2 {
        margin: 8px 0 0;
        font-size: var(--step-2);
    }

    .wait-actions {
        display: flex;
        gap: 10px;
        margin-top: 8px;
    }

    .remote-body {
        display: flex;
        min-height: 0;
        flex: 1;
        gap: 20px;
        padding: 20px 28px;
        overflow: hidden;
    }

    .viewer-column {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
        gap: 12px;
        overflow-y: auto;
    }

    .stage {
        display: flex;
        flex-direction: column;
        gap: 10px;
        padding: 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .stage-bar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }

    .share-label {
        color: var(--ink);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .quality {
        color: var(--ink-3);
        font-size: 12px;
    }

    .mock-screen {
        display: flex;
        aspect-ratio: 16 / 9;
        min-height: 300px;
        align-items: stretch;
        border-radius: var(--radius-s);
        background: var(--backdrop);
        overflow: hidden;
    }

    .mock-app {
        display: flex;
        flex-direction: column;
        flex: 1;
        padding: 12px;
        gap: 10px;
        color: var(--ink-3);
        font-size: 11px;
    }

    .mock-top {
        display: flex;
        align-items: center;
        gap: 5px;
        padding-bottom: 8px;
        border-bottom: 1px solid var(--line);
    }

    .mock-top > span:not(.mock-file) {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--line-strong);
    }

    .mock-file {
        margin-left: 8px;
        color: var(--ink-3);
    }

    .mock-columns {
        display: grid;
        grid-template-columns: 140px 1fr 150px;
        gap: 18px;
        min-height: 0;
        flex: 1;
    }

    .mock-side {
        display: flex;
        flex-direction: column;
        gap: 7px;
        padding-right: 12px;
        border-right: 1px solid var(--line);
    }

    .mock-side strong,
    .mock-meta strong {
        color: var(--ink-2);
        font-size: 11px;
        font-weight: 700;
    }

    .mock-side .label,
    .mock-meta .label {
        margin-top: 6px;
        color: var(--ink-3);
        font-size: 9px;
    }

    .mock-selected {
        padding: 2px 6px;
        border-radius: 4px;
        background: var(--surface-2);
        color: var(--ink);
    }

    .mock-main h3 {
        margin: 0 0 4px;
        color: var(--ink);
        font-size: 23px;
    }

    .mock-main p {
        margin: 0 0 16px;
        color: var(--ink-3);
    }

    .mock-main ul {
        display: flex;
        flex-direction: column;
        gap: 7px;
        margin: 0;
        padding-left: 18px;
        color: var(--ink-2);
    }

    .mock-meta {
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding-left: 12px;
        border-left: 1px solid var(--line);
    }

    .success {
        color: var(--success) !important;
    }

    .monitor-note {
        margin: 0;
        color: var(--ink-3);
        font-size: 12px;
    }

    .session-log {
        padding: 16px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .session-log h2 {
        margin: 0 0 10px;
    }

    .session-log div {
        display: grid;
        grid-template-columns: 70px 1fr 110px;
        gap: 10px;
        padding: 7px 0;
        color: var(--ink-2);
        font-size: 13px;
    }

    .session-log div span:last-child {
        color: var(--ink-3);
        text-align: right;
    }

    .side {
        width: 300px;
        flex-shrink: 0;
        overflow-y: auto;
    }

    .side-card {
        padding: 18px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .side-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        color: var(--ink-3);
        font-size: 12px;
    }

    .side-head h2 {
        margin: 0;
    }

    .share-title {
        margin: 10px 0 14px;
        color: var(--ink);
        font-size: 19px;
    }

    .peer-row {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 0;
        border-bottom: 1px solid var(--line);
    }

    .peer-row div {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .peer-row strong {
        font-size: 14px;
    }

    .peer-row span {
        color: var(--ink-3);
        font-size: 12px;
    }

    .option-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 14px 0;
        border-bottom: 1px solid var(--line);
    }

    .option-row div {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .option-row strong {
        font-size: 13px;
    }

    .option-row span {
        color: var(--ink-3);
        font-size: 12px;
    }

    .side-note {
        margin: 14px 0;
        color: var(--ink-3);
        font-size: 13px;
        line-height: 1.5;
    }

    .wide {
        width: 100%;
    }

    .viewer-controls {
        display: flex;
        justify-content: center;
        flex-shrink: 0;
        gap: 8px;
        padding-bottom: 20px;
    }
</style>

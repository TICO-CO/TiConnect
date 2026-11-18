<script lang="ts">
    import type { AppState } from '../lib';
    import { clock, fingerprintLabel, relativeSeen, shortName } from '../lib';
    import { firstVideoId } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    const details = $derived(app.details);
    const user = $derived(details?.user ?? app.userById(app.selectedUserId));
    const partyVideo = $derived(
        app.party !== null && app.party.user_id === user?.id ? app.party.video_id : '',
    );

    function startCall() {
        if (user !== null) {
            void app.startCall(user.id);
        }
    }

    function startRemote() {
        if (user !== null) {
            void app.requestRemote(user.id);
        }
    }

    function watchTogether() {
        if (user === null) {
            return;
        }
        const messageVideo = app.messages
            .map((message) => firstVideoId(message.body))
            .find((videoId) => videoId !== null);
        const videoId = partyVideo || messageVideo;
        if (!videoId) {
            app.notice = 'Open a conversation with a YouTube link first.';
            return;
        }
        void app.startParty(videoId, user.id);
    }

    function firstSeen(timestamp: number): string {
        if (timestamp <= 0) {
            return 'Unknown';
        }
        return new Date(timestamp).toLocaleDateString([], { day: '2-digit', month: 'short' });
    }

    function activityText(): string {
        if (app.call?.user_id === user?.id) {
            return 'Voice call active';
        }
        const remote = app.remote;
        if (remote !== null && remote.user_id === user?.id) {
            return remote.role === 'host' ? 'Sharing screen' : 'Viewing screen';
        }
        if (app.party?.user_id === user?.id) {
            return 'Watch party active';
        }
        return 'None';
    }
</script>

<section class="profile">
    <div class="back-row">
        <button
            class="icon-button small"
            onclick={() => app.backToChat()}
            aria-label="Back to conversation"
        >
            <Icon name="back" size={16} />
        </button>
    </div>
    {#if details && user}
        <header class="profile-head">
            <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={64} />
            <div class="identity">
                <h1 class="display">{shortName(user)}</h1>
                <p class="presence">
                    <span class="dot" class:online={user.connected}></span>
                    {user.connected ? 'Online' : 'Offline'}
                    <span class="separator">·</span>
                    Stable identity {fingerprintLabel(user.fingerprint)}
                </p>
                <p class="subtle">First seen {firstSeen(user.trusted_at)}</p>
            </div>
            <div class="actions">
                <button
                    class="btn btn-accent"
                    onclick={() => app.navigate({ kind: 'chat', id: user.id })}
                >
                    Message
                </button>
                <button class="btn" onclick={startCall}>
                    Call
                    <Icon name="call" size={15} />
                </button>
                <button class="btn" onclick={startRemote}>
                    Remote
                    <Icon name="remote" size={15} />
                </button>
                <button class="btn" onclick={watchTogether}>
                    Watch party
                    <Icon name="chevron-right" size={13} />
                </button>
            </div>
        </header>
        <div class="grid scroll">
            <div class="column">
                <section class="panel">
                    <h2 class="label">Connection</h2>
                    <dl>
                        <div>
                            <dt>Status</dt>
                            <dd class="online">{user.connected ? 'Online' : 'Offline'}</dd>
                        </div>
                        <div>
                            <dt>Current IP</dt>
                            <dd>{details.addresses || 'Not available'}</dd>
                        </div>
                        {#if app.addressChanged}
                            <div class="warning-row">
                                <dt>Previous IP</dt>
                                <dd>{app.preferences.previousAddresses[user.id] ?? 'Unknown'}</dd>
                            </div>
                            <p class="warning">Address changed recently</p>
                        {/if}
                        <div>
                            <dt>MAC</dt>
                            <dd>{details.mac || 'Not exposed'}</dd>
                        </div>
                        <div>
                            <dt>Connected since</dt>
                            <dd>
                                {app.connectedSince[user.id]
                                    ? clock(app.connectedSince[user.id] ?? 0)
                                    : 'Unknown'}
                            </dd>
                        </div>
                        <div>
                            <dt>Last seen</dt>
                            <dd>{relativeSeen(user.last_seen, user.connected)}</dd>
                        </div>
                        <div>
                            <dt>Transport</dt>
                            <dd>Encrypted · QUIC</dd>
                        </div>
                    </dl>
                </section>
                <section class="panel">
                    <h2 class="label">Device</h2>
                    <dl>
                        <div>
                            <dt>OS</dt>
                            <dd>{details.os || 'Unknown'}</dd>
                        </div>
                        <div>
                            <dt>Display server</dt>
                            <dd>{details.display_server || 'Unknown'}</dd>
                        </div>
                        <div>
                            <dt>App</dt>
                            <dd>{details.app_version || 'Unknown'}</dd>
                        </div>
                        <div>
                            <dt>Protocol</dt>
                            <dd>ticonnect/1</dd>
                        </div>
                    </dl>
                </section>
            </div>
            <div class="column">
                <section class="panel">
                    <h2 class="label">Activity</h2>
                    <dl>
                        <div>
                            <dt>Session</dt>
                            <dd>{activityText()}</dd>
                        </div>
                        {#each app.callLogs
                            .filter((log) => log.userId === user.id)
                            .slice(-3) as log (log.id)}
                            <div>
                                <dt>{log.reason === 'missed' ? 'Missed call' : 'Call'}</dt>
                                <dd>{clock(log.at)}</dd>
                            </div>
                        {/each}
                        <div>
                            <dt>Source</dt>
                            <dd>{details.source || 'LAN'}</dd>
                        </div>
                    </dl>
                </section>
                <section class="panel">
                    <h2 class="label">Policies</h2>
                    <div class="policy-row">
                        <span>Mute user</span>
                        <button
                            class="toggle"
                            aria-pressed={user.policy === 'mute'}
                            aria-label="Mute user"
                            onclick={() =>
                                void app.updatePolicy(
                                    user.id,
                                    user.policy === 'mute' ? 'accept' : 'mute',
                                )}
                        ></button>
                    </div>
                    <div class="policy-row">
                        <span>Ignore messages</span>
                        <button
                            class="toggle"
                            aria-pressed={user.policy === 'ignore'}
                            aria-label="Ignore messages"
                            onclick={() =>
                                void app.updatePolicy(
                                    user.id,
                                    user.policy === 'ignore' ? 'accept' : 'ignore',
                                )}
                        ></button>
                    </div>
                    <button
                        class="text-action"
                        onclick={() => (app.notice = 'Local history clearing is not wired yet.')}
                    >
                        Clear local history
                    </button>
                    <button class="btn btn-danger block" onclick={() => void app.blockUser(user.id)}
                        >Block user</button
                    >
                </section>
            </div>
        </div>
    {:else}
        <p class="empty">User details are not available.</p>
    {/if}
</section>

<style>
    .profile {
        display: flex;
        flex-direction: column;
        min-width: 0;
        height: 100%;
        flex: 1;
        background: var(--base);
        overflow: hidden;
    }

    .back-row {
        display: flex;
        align-items: center;
        height: 60px;
        flex-shrink: 0;
        padding-inline: 28px;
    }

    .profile-head {
        display: flex;
        align-items: center;
        gap: 16px;
        padding: 0 28px 20px;
        flex-shrink: 0;
    }

    .identity {
        display: flex;
        flex-direction: column;
        gap: 3px;
        min-width: 0;
        flex: 1;
    }

    .identity h1 {
        margin: 0;
        font-size: var(--step-3);
        line-height: 34px;
    }

    .presence {
        display: flex;
        align-items: center;
        gap: 6px;
        margin: 0;
        color: var(--ink-2);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .presence .separator {
        color: var(--ink-3);
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

    .identity .subtle {
        margin: 0;
        font-size: 13px;
    }

    .actions {
        display: flex;
        gap: 10px;
        flex-shrink: 0;
    }

    .grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        align-items: start;
        gap: 20px;
        padding: 0 28px 24px;
        min-height: 0;
        flex: 1;
        overflow-y: auto;
    }

    .column {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }

    .panel {
        padding: 18px 20px;
        background: var(--surface);
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
    }

    .panel h2 {
        margin: 0 0 12px;
    }

    dl {
        display: flex;
        flex-direction: column;
        gap: 0;
        margin: 0;
    }

    dl > div {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: 16px;
        padding: 9px 0;
        border-bottom: 1px solid color-mix(in srgb, var(--line) 55%, transparent);
    }

    dl > div:last-child {
        border-bottom: 0;
    }

    dt {
        color: var(--ink-3);
        font-size: 13px;
    }

    dd {
        margin: 0;
        color: var(--ink);
        font-size: 13px;
        text-align: right;
        overflow-wrap: anywhere;
    }

    dd.online {
        color: var(--success);
    }

    .warning-row dd {
        color: var(--warning);
    }

    .warning {
        margin: 6px 0 0;
        color: var(--warning);
        font-size: 12px;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .policy-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 10px 0;
        border-bottom: 1px solid color-mix(in srgb, var(--line) 55%, transparent);
    }

    .text-action {
        margin-top: 12px;
        padding: 0;
        border: 0;
        background: transparent;
        color: var(--ink);
        font-size: 14px;
        font-weight: 700;
    }

    .block {
        width: 100%;
        margin-top: 12px;
    }

    .empty {
        padding: 32px;
    }
</style>

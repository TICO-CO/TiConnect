<script lang="ts">
    import { onMount } from 'svelte';

    import type { Preferences } from '../lib';
    import type { AppState } from '../lib';
    import { fingerprintLabel } from '../lib';
    import { DEFAULT_PREFERENCES } from '../lib';
    import { SETTINGS_SECTIONS, type SettingsSection } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let avatarInput: HTMLInputElement | null = $state(null);
    let preferencesDraft = $state<Preferences>({ ...DEFAULT_PREFERENCES });
    onMount(() => {
        preferencesDraft = { ...app.preferences };
    });
    const section = $derived<SettingsSection>(
        app.route.kind === 'settings' ? app.route.section : 'user',
    );
    const sectionLabel = $derived(
        SETTINGS_SECTIONS.find((item) => item.id === section)?.label ?? 'Settings',
    );
    const subtitle = $derived(
        {
            user: 'Your identity on this network',
            notifications: 'Do not disturb, overlay and message toasts',
            calls: 'Ringtones, devices and ringing timeout',
            remote: 'Consent, session limits and video quality',
            media: 'YouTube overlay, incoming videos, watch parties',
            storage: 'Outbox, attachments and local history',
            about: 'Version, identity and protocol',
        }[section],
    );

    function value(key: string, fallback = ''): string {
        return app.settingsDraft[key] ?? fallback;
    }

    function enabled(key: string, fallback = false): boolean {
        const stored = app.settingsDraft[key];
        if (stored === undefined) {
            return fallback;
        }
        return stored === 'true';
    }

    function setValue(key: string, next: string): void {
        app.setDraftSetting(key, next);
    }

    function setEnabled(key: string, next: boolean): void {
        app.setDraftSetting(key, String(next));
    }

    function setPreference<K extends keyof Preferences>(key: K, next: Preferences[K]): void {
        preferencesDraft = { ...preferencesDraft, [key]: next };
    }

    function save(): void {
        app.updatePreferences(preferencesDraft);
        void app.saveSettings();
    }

    function revert(): void {
        app.revertSettings();
        preferencesDraft = { ...app.preferences };
    }

    function uploadAvatar(event: Event): void {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (file) {
            void app.uploadAvatar(file);
        }
        input.value = '';
    }
</script>

<section class="settings">
    <aside class="settings-rail">
        <div class="settings-top">
            <button class="icon-button small" onclick={() => app.backToChat()} aria-label="Back">
                <Icon name="back" size={16} />
            </button>
        </div>
        <nav aria-label="Settings sections">
            {#each SETTINGS_SECTIONS as item (item.id)}
                <button
                    class="settings-tab"
                    class:active={section === item.id}
                    onclick={() => app.navigate({ kind: 'settings', section: item.id })}
                    aria-current={section === item.id ? 'page' : undefined}
                >
                    {item.label}
                </button>
            {/each}
        </nav>
    </aside>
    <div class="settings-content">
        <header class="settings-head">
            <h1 class="display">{sectionLabel}</h1>
            <p>{subtitle}</p>
        </header>
        <div class="settings-scroll scroll">
            {#if section === 'user'}
                <section class="panel profile-card">
                    <h2 class="label">Profile card</h2>
                    <div class="profile-body">
                        <button
                            class="avatar-upload"
                            onclick={() => avatarInput?.click()}
                            aria-label="Change avatar"
                        >
                            <Avatar
                                name={app.bootstrap?.identity.display_name ?? 'You'}
                                size={196}
                            />
                            <span class="avatar-overlay">Change</span>
                        </button>
                        <input
                            class="sr-only"
                            type="file"
                            accept="image/png,image/jpeg"
                            bind:this={avatarInput}
                            onchange={uploadAvatar}
                        />
                        <div class="profile-copy">
                            <h3 class="display">
                                {app.bootstrap?.identity.display_name || 'TiConnect device'}
                            </h3>
                            <p class="presence">
                                <span class="dot"></span>Online · visible on the network
                            </p>
                            <p class="profile-description">
                                {preferencesDraft.userDescription || 'No description yet.'}
                            </p>
                        </div>
                        <div class="profile-fields">
                            <label class="stack">
                                <span class="label">Username</span>
                                <input
                                    class="field"
                                    value={value('identity.display_name')}
                                    oninput={(event) =>
                                        setValue(
                                            'identity.display_name',
                                            (event.currentTarget as HTMLInputElement).value,
                                        )}
                                />
                            </label>
                            <label class="stack">
                                <span class="label">Description</span>
                                <textarea
                                    class="field"
                                    rows="3"
                                    value={preferencesDraft.userDescription}
                                    oninput={(event) =>
                                        setPreference(
                                            'userDescription',
                                            (event.currentTarget as HTMLTextAreaElement).value,
                                        )}></textarea>
                            </label>
                        </div>
                    </div>
                </section>
                <section class="panel">
                    <h2 class="label">Presence</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Default presence</strong><span
                                >Holds until you change it from the shell</span
                            >
                        </div>
                        <div class="segmented">
                            {#each ['online', 'away', 'dnd'] as presence (presence)}
                                <button
                                    aria-pressed={preferencesDraft.presence === presence}
                                    onclick={() => setPreference('presence', presence)}
                                >
                                    {presence === 'dnd'
                                        ? 'Do not disturb'
                                        : presence === 'away'
                                          ? 'Away'
                                          : 'Online'}
                                </button>
                            {/each}
                        </div>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Read receipts</strong><span
                                >Lets contacts see when you have read a message</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Read receipts"
                            aria-pressed={enabled('read_receipts.enabled', true)}
                            onclick={() =>
                                setEnabled(
                                    'read_receipts.enabled',
                                    !enabled('read_receipts.enabled', true),
                                )}
                        ></button>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Typing indicators</strong><span
                                >Shows when you are composing a reply</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Typing indicators"
                            aria-pressed={enabled('typing.enabled', true)}
                            onclick={() =>
                                setEnabled('typing.enabled', !enabled('typing.enabled', true))}
                        ></button>
                    </div>
                </section>
                <section class="panel">
                    <div class="row header-row">
                        <h2 class="label">Identity key</h2>
                        <span class="label">Read-only</span>
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Stable identity</span></div>
                        <strong
                            >{fingerprintLabel(app.bootstrap?.identity.fingerprint ?? '')}</strong
                        >
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Key</span></div>
                        <strong>{app.bootstrap?.identity.fingerprint.slice(0, 8) ?? ''}</strong>
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>First seen</span></div>
                        <strong>12 Oct</strong>
                    </div>
                </section>
            {:else if section === 'notifications'}
                <section class="panel">
                    <h2 class="label">Notifications</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Do not disturb</strong><span
                                >Incoming messages and calls stay quiet</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Do not disturb"
                            aria-pressed={enabled('dnd.enabled')}
                            onclick={() => setEnabled('dnd.enabled', !enabled('dnd.enabled'))}
                        ></button>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Desktop overlay</strong><span
                                >Corner cards while the window is closed</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Desktop overlay"
                            aria-pressed={enabled('overlay.enabled', true)}
                            onclick={() =>
                                setEnabled('overlay.enabled', !enabled('overlay.enabled', true))}
                        ></button>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Overlay timeout</strong><span
                                >Seconds before a corner card dismisses</span
                            >
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="1"
                            max="60"
                            value={value('overlay.timeout_secs', '5')}
                            oninput={(event) =>
                                setValue(
                                    'overlay.timeout_secs',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Message sounds</strong><span>Soft chime for new messages</span>
                        </div>
                        <button
                            class="toggle"
                            aria-label="Message sounds"
                            aria-pressed={preferencesDraft.sounds}
                            onclick={() => setPreference('sounds', !preferencesDraft.sounds)}
                        ></button>
                    </div>
                </section>
            {:else if section === 'calls'}
                <section class="panel">
                    <h2 class="label">Calls</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Ringing timeout</strong><span
                                >Seconds before an unanswered call ends</span
                            >
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="5"
                            max="300"
                            value={value('calls.timeout_secs', '30')}
                            oninput={(event) =>
                                setValue(
                                    'calls.timeout_secs',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Input device</strong><span
                                >Leave empty for the system default</span
                            >
                        </div>
                        <input
                            class="field text"
                            value={value('calls.input_device')}
                            oninput={(event) =>
                                setValue(
                                    'calls.input_device',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Output device</strong><span
                                >Leave empty for the system default</span
                            >
                        </div>
                        <input
                            class="field text"
                            value={value('calls.output_device')}
                            oninput={(event) =>
                                setValue(
                                    'calls.output_device',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Ringtone path</strong><span>Empty uses the bundled tone</span>
                        </div>
                        <input
                            class="field text"
                            value={value('ringtone.path')}
                            oninput={(event) =>
                                setValue(
                                    'ringtone.path',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                </section>
            {:else if section === 'remote'}
                <section class="panel">
                    <h2 class="label">Remote desktop</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Session timeout</strong><span>0 disables automatic timeout</span
                            >
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="0"
                            value={value('remote.session_timeout_secs', '0')}
                            oninput={(event) =>
                                setValue(
                                    'remote.session_timeout_secs',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Request expiry</strong><span
                                >Seconds a consent request stays valid</span
                            >
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="30"
                            max="3600"
                            value={value('remote.request_timeout_secs', '300')}
                            oninput={(event) =>
                                setValue(
                                    'remote.request_timeout_secs',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <p class="hint">Accepting a request is the full consent grant.</p>
                </section>
                <section class="panel">
                    <h2 class="label">Video quality</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Bitrate</strong><span>Kilobits per second</span>
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="256"
                            max="20000"
                            value={value('video.bitrate_kbps', '2500')}
                            oninput={(event) =>
                                setValue(
                                    'video.bitrate_kbps',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Frame rate</strong><span>Frames per second</span>
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="5"
                            max="60"
                            value={value('video.fps', '30')}
                            oninput={(event) =>
                                setValue(
                                    'video.fps',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Keyframe interval</strong><span>Frames between keyframes</span>
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="1"
                            max="600"
                            value={value('video.keyframe_interval', '60')}
                            oninput={(event) =>
                                setValue(
                                    'video.keyframe_interval',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                </section>
            {:else if section === 'media'}
                <section class="panel">
                    <h2 class="label">Playback</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Autoplay YouTube links</strong><span
                                >Sent and received links open the player immediately</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Autoplay YouTube links"
                            aria-pressed={preferencesDraft.autoplay}
                            onclick={() => setPreference('autoplay', !preferencesDraft.autoplay)}
                        ></button>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Queue limit</strong><span
                                >New links queue behind the current video</span
                            >
                        </div>
                        <div class="segmented">
                            {#each [1, 3, 5] as limit (limit)}
                                <button
                                    aria-pressed={preferencesDraft.queueLimit === limit}
                                    onclick={() => setPreference('queueLimit', limit)}
                                    >{limit}</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Player position</strong><span
                                >Bottom-right, above message toasts</span
                            >
                        </div>
                        <span class="row-value">Fixed</span>
                    </div>
                </section>
                <section class="panel">
                    <h2 class="label">Incoming videos</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Ignore all incoming videos</strong><span
                                >Links stay in chat until you reactivate</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Ignore all incoming videos"
                            aria-pressed={preferencesDraft.ignoreIncoming}
                            onclick={() =>
                                setPreference('ignoreIncoming', !preferencesDraft.ignoreIncoming)}
                        ></button>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Dismiss player</strong><span
                                >Closing keeps the video in chat and queue</span
                            >
                        </div>
                        <span class="row-value">{preferencesDraft.dismissPlayer}</span>
                    </div>
                </section>
                <section class="panel">
                    <h2 class="label">Watch parties</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Playback control</strong><span
                                >Anyone in the party can play, pause and seek</span
                            >
                        </div>
                        <div class="segmented">
                            {#each ['Everyone', 'Host'] as control (control)}
                                <button
                                    aria-pressed={preferencesDraft.partyControl === control}
                                    onclick={() => setPreference('partyControl', control)}
                                    >{control}</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Accept party invites</strong><span
                                >Invites appear top-right</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Accept party invites"
                            aria-pressed={preferencesDraft.acceptInvites}
                            onclick={() =>
                                setPreference('acceptInvites', !preferencesDraft.acceptInvites)}
                        ></button>
                    </div>
                </section>
            {:else if section === 'storage'}
                <section class="panel">
                    <h2 class="label">Storage</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Outbox retention</strong><span
                                >Days before undelivered records expire</span
                            >
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="1"
                            max="365"
                            value={value('outbox.ttl_days', '7')}
                            oninput={(event) =>
                                setValue(
                                    'outbox.ttl_days',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Attachment limit</strong><span>Maximum size in MiB</span>
                        </div>
                        <input
                            class="field number"
                            type="number"
                            min="1"
                            max="102400"
                            value={value('attachments.max_size_mib', '2048')}
                            oninput={(event) =>
                                setValue(
                                    'attachments.max_size_mib',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Local history</strong><span
                                >Keep messages and attachments for</span
                            >
                        </div>
                        <div class="segmented">
                            {#each [7, 30, 90] as days (days)}
                                <button
                                    aria-pressed={preferencesDraft.storageRetention === days}
                                    onclick={() => setPreference('storageRetention', days)}
                                    >{days} d</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Local usage</strong><span>Attachments and message database</span
                            >
                        </div>
                        <span class="row-value">Placeholder · not measured</span>
                    </div>
                    <button
                        class="btn btn-danger wide"
                        onclick={() => (app.notice = 'Clearing local history is not wired yet.')}
                        >Clear local history</button
                    >
                </section>
            {:else}
                <section class="panel">
                    <h2 class="label">About TiConnect</h2>
                    <div class="row">
                        <div class="row-copy"><span>Version</span></div>
                        <strong>{app.bootstrap?.version ?? '0.0.6'}</strong>
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Protocol</span></div>
                        <strong>ticonnect/1</strong>
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Identity</span></div>
                        <strong
                            >{fingerprintLabel(app.bootstrap?.identity.fingerprint ?? '')}</strong
                        >
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Network</span></div>
                        <strong
                            >{app.users.filter((user) => user.connected).length} online · LAN only</strong
                        >
                    </div>
                    <div class="row">
                        <div class="row-copy"><span>Licence</span></div>
                        <strong>GPL-3.0-or-later</strong>
                    </div>
                </section>
                <section class="panel">
                    <h2 class="label">Discovery</h2>
                    <div class="row">
                        <div class="row-copy">
                            <strong>mDNS</strong><span>_ticonnect._udp.local.</span>
                        </div>
                        <span class="row-value"
                            >{enabled('discovery.mdns', true) ? 'On' : 'Off'}</span
                        >
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Broadcast fallback</strong><span>UDP beacon on the LAN</span>
                        </div>
                        <span class="row-value"
                            >{enabled('discovery.broadcast', true) ? 'On' : 'Off'}</span
                        >
                    </div>
                    <div class="row">
                        <div class="row-copy">
                            <strong>Beacon interval</strong><span>Seconds</span>
                        </div>
                        <span class="row-value">{value('discovery.beacon_interval_secs', '5')}</span
                        >
                    </div>
                </section>
            {/if}
        </div>
        <footer class="settings-actions">
            <button class="btn" onclick={revert}>Revert</button>
            <button class="btn btn-accent" onclick={save}>Save changes</button>
        </footer>
    </div>
</section>

<style>
    .settings {
        display: flex;
        height: 100%;
        min-width: 0;
        flex: 1;
        background: var(--base);
    }

    .settings-rail {
        display: flex;
        flex-direction: column;
        width: 320px;
        flex-shrink: 0;
        height: 100%;
        padding: 12px;
        gap: 6px;
        background: var(--rail);
    }

    .settings-top {
        display: flex;
        align-items: center;
        height: 30px;
        padding-inline: 4px;
    }

    nav {
        display: flex;
        flex-direction: column;
        gap: 2px;
        margin-top: 14px;
    }

    .settings-tab {
        display: flex;
        align-items: center;
        height: 42px;
        flex-shrink: 0;
        padding-inline: 14px;
        border: 0;
        border-radius: var(--radius-s);
        background: transparent;
        color: var(--ink-2);
        font-size: 15px;
        font-weight: 700;
        text-align: left;
    }

    .settings-tab:hover {
        background: var(--surface);
        color: var(--ink);
    }

    .settings-tab.active {
        width: 320px;
        margin-inline: -12px;
        padding-inline: 26px;
        border-radius: 0;
        background: var(--ink);
        color: var(--base);
    }

    .settings-content {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
        padding: 24px 32px 32px;
        overflow: hidden;
    }

    .settings-head {
        flex-shrink: 0;
        margin-bottom: 16px;
    }

    .settings-head h1 {
        margin: 0;
        font-size: var(--step-3);
        line-height: 34px;
    }

    .settings-head p {
        margin: 4px 0 0;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .settings-scroll {
        display: flex;
        flex-direction: column;
        min-height: 0;
        flex: 1;
        gap: 16px;
        overflow-y: auto;
        padding-right: 4px;
    }

    .panel {
        padding: 16px 20px;
    }

    .panel h2 {
        margin: 0 0 12px;
    }

    .profile-body {
        display: flex;
        align-items: stretch;
        gap: 20px;
    }

    .avatar-upload {
        position: relative;
        width: 196px;
        height: 196px;
        flex-shrink: 0;
        overflow: hidden;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-m);
        background: var(--surface-2);
        padding: 0;
    }

    .avatar-overlay {
        position: absolute;
        inset: auto 0 0;
        padding: 8px;
        background: rgba(15, 31, 34, 0.78);
        color: var(--ink);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .profile-copy {
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 8px;
        min-width: 0;
        flex: 1;
    }

    .profile-copy h3 {
        margin: 0;
        font-size: var(--step-2);
    }

    .presence {
        display: flex;
        align-items: center;
        gap: 6px;
        margin: 0;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--success);
    }

    .profile-description {
        max-width: 340px;
        margin: 4px 0 0;
        color: var(--ink-2);
        font-size: 13px;
        line-height: 1.45;
    }

    .profile-fields {
        display: flex;
        flex-direction: column;
        gap: 12px;
        width: 390px;
        flex-shrink: 0;
        justify-content: center;
    }

    .stack {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }

    .row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 24px;
        padding: 9px 0;
        border-bottom: 1px solid color-mix(in srgb, var(--line) 55%, transparent);
    }

    .row:last-child {
        border-bottom: 0;
    }

    .row-copy {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .row-copy strong {
        font-size: 14px;
    }

    .row-copy span {
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .row-value {
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .number {
        width: 96px;
        flex-shrink: 0;
    }

    .text {
        width: 320px;
        flex-shrink: 0;
    }

    .header-row {
        padding-top: 0;
    }

    .hint {
        margin: 12px 0 0;
        color: var(--ink-3);
        font-size: 13px;
    }

    .wide {
        width: 100%;
        margin-top: 12px;
    }

    .settings-actions {
        display: flex;
        justify-content: flex-end;
        flex-shrink: 0;
        gap: 10px;
        padding-top: 20px;
    }
</style>

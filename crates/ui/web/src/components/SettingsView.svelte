<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState, AudioDevice, ConfirmRequest, Preferences } from '../lib';
    import { dateLabel, fileSize, fingerprintLabel } from '../lib';
    import { DEFAULT_PREFERENCES } from '../lib';
    import { routeHash, SETTINGS_SECTIONS, type SettingsSection } from '../lib';
    import Avatar from './Avatar.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import Icon from './Icon.svelte';
    import StateBanner from './StateBanner.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let avatarInput: HTMLInputElement | null = $state(null);
    let preferencesDraft = $state<Preferences>({ ...DEFAULT_PREFERENCES });
    let confirmRequest = $state<ConfirmRequest | null>(null);
    let confirmPending = $state(false);
    let confirmError = $state('');
    onMount(() => {
        preferencesDraft = { ...app.preferences };
    });
    const section = $derived<SettingsSection>(
        app.route.kind === 'settings' ? app.route.section : 'user',
    );
    const sectionLabel = $derived(
        SETTINGS_SECTIONS.find((item) => item.id === section)?.label ?? 'Settings',
    );
    const inputDevices = $derived(deviceOptions('input'));
    const outputDevices = $derived(deviceOptions('output'));
    const subtitle = $derived(
        {
            user: 'Your identity on this network',
            notifications: 'Do not disturb, overlay and message toasts',
            calls: 'Shortcuts and ringing timeout',
            audio: 'Microphone and speaker levels',
            remote: 'Consent, session limits and video quality',
            media: 'YouTube overlay, incoming videos, watch parties',
            storage: 'Outbox, attachments and local history',
            about: 'Version, identity and protocol',
        }[section],
    );
    const preferencesDirty = $derived(
        JSON.stringify(preferencesDraft) !== JSON.stringify(app.preferences),
    );
    const dirty = $derived(app.settingsDirty || preferencesDirty);
    const saveStatus = $derived.by(() => {
        if (app.settingsSave === 'saving') {
            return { text: 'Saving changes…', style: 'text-ink-3' };
        }
        if (app.settingsSave === 'error') {
            return { text: 'Changes not saved', style: 'text-error' };
        }
        if (dirty) {
            return { text: 'Unsaved changes', style: 'text-warning' };
        }
        if (app.settingsSave === 'saved') {
            return { text: 'All changes saved', style: 'text-success' };
        }
        return null;
    });

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

    function deviceOptions(kind: string): AudioDevice[] {
        const devices = app.audioDevices.filter(
            (device, index, all) =>
                device.kind === kind &&
                all.findIndex(
                    (candidate) => candidate.kind === kind && candidate.id === device.id,
                ) === index,
        );
        const current = value(`calls.${kind}_device`);
        if (current === '' || devices.some((device) => device.id === current)) {
            return devices;
        }
        return [{ id: current, name: `Unavailable · ${current}`, kind }, ...devices];
    }

    function setEnabled(key: string, next: boolean): void {
        app.setDraftSetting(key, String(next));
    }

    function setAppearance(next: string): void {
        setValue('appearance.color_scheme', next);
        app.applyAppearance();
    }

    function requestClearHistory(userId = ''): void {
        const scope = userId === '' ? 'all local messages and attachments' : 'this conversation';
        confirmError = '';
        confirmRequest = {
            title: 'Clear local history',
            body: `Clear ${scope}? This cannot be undone.`,
            confirmLabel: 'Clear history',
            cancelLabel: 'Cancel',
            tone: 'error',
            action: () => app.clearHistory(userId),
        };
    }

    function closeConfirm(): void {
        if (confirmPending) {
            return;
        }
        confirmRequest = null;
        confirmError = '';
    }

    async function runConfirm(): Promise<void> {
        if (confirmRequest === null) {
            return;
        }
        confirmPending = true;
        confirmError = '';
        await confirmRequest.action();
        confirmPending = false;
        confirmRequest = null;
    }

    function setPreference<K extends keyof Preferences>(key: K, next: Preferences[K]): void {
        preferencesDraft = { ...preferencesDraft, [key]: next };
        app.markSettingsDirty();
    }

    function lanStateLabel(state: string): string {
        switch (state) {
            case 'disabled':
                return 'Disabled';
            case 'no_interface':
                return 'No LAN interface';
            case 'bind_failed':
                return 'Port unavailable';
            case 'discovery_down':
                return 'Discovery unavailable';
            case 'no_peers':
                return 'No peers found';
            case 'peers_discovered':
                return 'Peers found';
            case 'peers_unreachable':
                return 'Peers unreachable';
            case 'reachable':
                return 'Reachable';
            default:
                return 'Unknown';
        }
    }

    function lanStateClass(state: string): string {
        if (state === 'reachable') {
            return 'text-success';
        }
        if (state === 'no_peers' || state === 'peers_discovered' || state === 'unknown') {
            return 'text-ink-2';
        }
        return 'text-warning';
    }

    async function save(): Promise<void> {
        app.updatePreferences(preferencesDraft);
        await app.saveSettings();
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

<section class="flex h-full min-w-0 flex-1 bg-base">
    <aside class="flex h-full w-80 shrink-0 flex-col gap-1.5 bg-rail p-3">
        <div class="flex h-[30px] items-center px-1">
            <button
                class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                onclick={() => app.backToChat()}
                aria-label="Back"
            >
                <Icon name="back" size={16} />
            </button>
        </div>
        <nav class="mt-3.5 flex flex-col gap-0.5" aria-label="Settings sections">
            {#each SETTINGS_SECTIONS as item (item.id)}
                <a
                    class="-mx-3 flex h-[42px] w-80 shrink-0 items-center rounded-none px-[26px] text-left text-[15px] font-bold text-ink-2 no-underline hover:bg-surface hover:text-ink aria-[current=page]:bg-ink aria-[current=page]:text-base aria-[current=page]:hover:bg-ink aria-[current=page]:hover:text-base"
                    href={routeHash({ kind: 'settings', section: item.id })}
                    onclick={(event) => {
                        event.preventDefault();
                        app.navigate({ kind: 'settings', section: item.id });
                    }}
                    aria-current={section === item.id ? 'page' : undefined}
                >
                    {item.label}
                </a>
            {/each}
        </nav>
    </aside>
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden px-8 pt-6 pb-8">
        <header class="mb-4 shrink-0">
            <h1 class="m-0 font-display text-step-3 leading-[34px] font-normal tracking-[-0.01em]">
                {sectionLabel}
            </h1>
            <p class="m-0 mt-1 text-xs tracking-[0.06em] text-ink-3 uppercase">{subtitle}</p>
        </header>
        <div
            class="flex scroll min-h-0 flex-1 flex-col gap-4 overflow-x-hidden overflow-y-auto pr-1"
        >
            {#if section === 'user'}
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Profile card
                    </h2>
                    <div class="flex items-stretch gap-5">
                        <button
                            class="relative size-[196px] shrink-0 overflow-hidden rounded-md border border-line-strong bg-surface-2 p-0"
                            onclick={() => avatarInput?.click()}
                            aria-label="Change avatar"
                        >
                            <Avatar
                                image={app.selfAvatarUrl ||
                                    app.avatarUrl(app.bootstrap?.identity.fingerprint ?? '')}
                                name={app.bootstrap?.identity.display_name ?? 'You'}
                                size={196}
                            />
                            <span
                                class="absolute inset-x-0 top-auto bottom-0 bg-backdrop/78 p-2 text-xs tracking-[0.06em] text-ink uppercase"
                                >Change</span
                            >
                        </button>
                        <input
                            class="sr-only"
                            type="file"
                            accept="image/png,image/jpeg"
                            bind:this={avatarInput}
                            onchange={uploadAvatar}
                        />
                        <div class="flex min-w-0 flex-1 flex-col justify-center gap-2">
                            <h3
                                class="m-0 font-display text-step-2 leading-[1.15] font-normal tracking-[-0.01em]"
                            >
                                {app.bootstrap?.identity.display_name || 'TiConnect device'}
                            </h3>
                            <p
                                class="m-0 flex items-center gap-1.5 text-xs tracking-[0.06em] text-ink-3 uppercase"
                            >
                                <span class="size-1.5 rounded-full bg-success"></span>Online ·
                                visible on the network
                            </p>
                            <p
                                class="m-0 mt-1 max-w-[340px] text-step-xs leading-[1.45] text-ink-2"
                            >
                                {preferencesDraft.userDescription || 'No description yet.'}
                            </p>
                        </div>
                        <div class="flex w-[390px] shrink-0 flex-col justify-center gap-3">
                            <label class="flex flex-col gap-1.5">
                                <span
                                    class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                                    >Username</span
                                >
                                <input
                                    class="min-h-9 w-full rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
                                    value={value('identity.display_name')}
                                    oninput={(event) =>
                                        setValue(
                                            'identity.display_name',
                                            (event.currentTarget as HTMLInputElement).value,
                                        )}
                                />
                            </label>
                            <label class="flex flex-col gap-1.5">
                                <span
                                    class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                                    >Description</span
                                >
                                <textarea
                                    class="min-h-9 w-full rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Appearance
                    </h2>
                    <div class="flex items-center justify-between gap-6">
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Color scheme</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >System follows the desktop preference</span
                            >
                        </div>
                        <div class="inline-flex shrink-0 gap-1">
                            {#each ['system', 'light', 'dark'] as appearance (appearance)}
                                <button
                                    class="min-h-8 rounded-pill border border-line bg-transparent px-3.5 text-xs font-bold text-ink-2 capitalize aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-base"
                                    aria-pressed={value('appearance.color_scheme', 'system') ===
                                        appearance}
                                    onclick={() => setAppearance(appearance)}>{appearance}</button
                                >
                            {/each}
                        </div>
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Presence
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Default presence</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Holds until you change it from the shell</span
                            >
                        </div>
                        <div class="inline-flex shrink-0 gap-1">
                            {#each ['online', 'away', 'dnd'] as presence (presence)}
                                <button
                                    class="min-h-8 rounded-pill border border-line bg-transparent px-3.5 text-xs font-bold text-ink-2 aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-base"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Read receipts</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Typing indicators</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 pb-[9px]"
                    >
                        <h2
                            class="m-0 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                        >
                            Identity key
                        </h2>
                        <span
                            class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                            >Read-only</span
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Stable identity</span></div>
                        <strong
                            >{fingerprintLabel(app.bootstrap?.identity.fingerprint ?? '')}</strong
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Key</span></div>
                        <strong>{app.bootstrap?.identity.fingerprint.slice(0, 8) ?? ''}</strong>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>First seen</span></div>
                        <strong>{dateLabel(app.bootstrap?.identity.created_at ?? 0)}</strong>
                    </div>
                </section>
            {:else if section === 'notifications'}
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Notifications
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Do not disturb</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Desktop overlay</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Overlay timeout</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Seconds before a corner card dismisses</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Message sounds</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Soft chime for new messages</span
                            >
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
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Calls
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Ringing timeout</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Seconds before an unanswered call ends</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Mute shortcut</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Example: Ctrl+M. Empty disables it</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-80 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
                            value={value('calls.mute_shortcut', 'Ctrl+M')}
                            oninput={(event) =>
                                setValue(
                                    'calls.mute_shortcut',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Output mute shortcut</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Example: Ctrl+Shift+M. Empty disables it</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-80 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
                            value={value('calls.output_mute_shortcut', 'Ctrl+Shift+M')}
                            oninput={(event) =>
                                setValue(
                                    'calls.output_mute_shortcut',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                </section>
            {:else if section === 'audio'}
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Input
                    </h2>
                    <div
                        class="flex min-w-0 items-center justify-between gap-6 border-b border-line/55 py-[9px]"
                    >
                        <div class="flex min-w-0 flex-col gap-0.5">
                            <strong class="text-sm">Microphone</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Leave empty for the system default</span
                            >
                        </div>
                        <div class="w-80 max-w-full min-w-0 shrink-0 overflow-hidden">
                            <select
                                class="min-h-9 w-full min-w-0 rounded-sm border border-line bg-base px-3 py-2 text-ink hover:border-line-strong"
                                aria-label="Microphone"
                                value={value('calls.input_device')}
                                onchange={(event) =>
                                    setValue(
                                        'calls.input_device',
                                        (event.currentTarget as HTMLSelectElement).value,
                                    )}
                            >
                                <option value="">System default</option>
                                {#each inputDevices as device (device.id)}
                                    <option value={device.id}>{device.name}</option>
                                {/each}
                            </select>
                        </div>
                    </div>
                    <div class="flex flex-col gap-2 py-[9px]">
                        <div class="flex items-center justify-between gap-6">
                            <strong class="text-sm">Input volume</strong>
                            <span class="text-step-xs text-ink-2 tabular-nums"
                                >{value('audio.input_volume', '100')}%</span
                            >
                        </div>
                        <input
                            class="h-8 w-full accent-accent"
                            type="range"
                            min="0"
                            max="100"
                            step="1"
                            value={value('audio.input_volume', '100')}
                            aria-label="Input volume"
                            oninput={(event) =>
                                setValue(
                                    'audio.input_volume',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                        <span class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                            >Applied to calls and voice messages</span
                        >
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Output
                    </h2>
                    <div
                        class="flex min-w-0 items-center justify-between gap-6 border-b border-line/55 py-[9px]"
                    >
                        <div class="flex min-w-0 flex-col gap-0.5">
                            <strong class="text-sm">Speakers</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Leave empty for the system default</span
                            >
                        </div>
                        <div class="w-80 max-w-full min-w-0 shrink-0 overflow-hidden">
                            <select
                                class="min-h-9 w-full min-w-0 rounded-sm border border-line bg-base px-3 py-2 text-ink hover:border-line-strong"
                                aria-label="Speakers"
                                value={value('calls.output_device')}
                                onchange={(event) =>
                                    setValue(
                                        'calls.output_device',
                                        (event.currentTarget as HTMLSelectElement).value,
                                    )}
                            >
                                <option value="">System default</option>
                                {#each outputDevices as device (device.id)}
                                    <option value={device.id}>{device.name}</option>
                                {/each}
                            </select>
                        </div>
                    </div>
                    <div class="flex flex-col gap-2 py-[9px]">
                        <div class="flex items-center justify-between gap-6">
                            <strong class="text-sm">Output volume</strong>
                            <span class="text-step-xs text-ink-2 tabular-nums"
                                >{value('audio.output_volume', '100')}%</span
                            >
                        </div>
                        <input
                            class="h-8 w-full accent-accent"
                            type="range"
                            min="0"
                            max="100"
                            step="1"
                            value={value('audio.output_volume', '100')}
                            aria-label="Output volume"
                            oninput={(event) =>
                                setValue(
                                    'audio.output_volume',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                        <span class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                            >Per-user levels scale this output</span
                        >
                    </div>
                </section>
            {:else if section === 'remote'}
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Remote desktop
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Session timeout</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >0 disables automatic timeout</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Request expiry</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Seconds a consent request stays valid</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <p class="m-0 mt-3 text-step-xs text-ink-3">
                        Accepting a request is the full consent grant.
                    </p>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Video quality
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Bitrate</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Kilobits per second</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Frame rate</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Frames per second</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Keyframe interval</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Frames between keyframes</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Playback
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Autoplay YouTube links</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Queue limit</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >New links queue behind the current video</span
                            >
                        </div>
                        <div class="inline-flex shrink-0 gap-1">
                            {#each [1, 3, 5] as limit (limit)}
                                <button
                                    class="min-h-8 rounded-pill border border-line bg-transparent px-3.5 text-xs font-bold text-ink-2 aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-base"
                                    aria-pressed={preferencesDraft.queueLimit === limit}
                                    onclick={() => setPreference('queueLimit', limit)}
                                    >{limit}</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Player position</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Bottom-right, above message toasts</span
                            >
                        </div>
                        <span class="text-xs tracking-[0.06em] text-ink-3 uppercase">Fixed</span>
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Incoming videos
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Ignore all incoming videos</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Dismiss player</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Closing keeps the video in chat and queue</span
                            >
                        </div>
                        <span class="text-xs tracking-[0.06em] text-ink-3 uppercase"
                            >{preferencesDraft.dismissPlayer}</span
                        >
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Watch parties
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Playback control</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Anyone in the party can play, pause and seek</span
                            >
                        </div>
                        <div class="inline-flex shrink-0 gap-1">
                            {#each ['Everyone', 'Host'] as control (control)}
                                <button
                                    class="min-h-8 rounded-pill border border-line bg-transparent px-3.5 text-xs font-bold text-ink-2 aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-base"
                                    aria-pressed={preferencesDraft.partyControl === control}
                                    onclick={() => setPreference('partyControl', control)}
                                    >{control}</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Accept party invites</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
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
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Storage
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Outbox retention</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Days before undelivered records expire</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Attachment limit</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Maximum size in MiB</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
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
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Local history</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Keep messages and attachments for</span
                            >
                        </div>
                        <div class="inline-flex shrink-0 gap-1">
                            {#each [7, 30, 90] as days (days)}
                                <button
                                    class="min-h-8 rounded-pill border border-line bg-transparent px-3.5 text-xs font-bold text-ink-2 aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-base"
                                    aria-pressed={preferencesDraft.storageRetention === days}
                                    onclick={() => setPreference('storageRetention', days)}
                                    >{days} d</button
                                >
                            {/each}
                        </div>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Local usage</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Attachments and message database</span
                            >
                        </div>
                        <span class="text-right text-xs tracking-[0.06em] text-ink-3 uppercase"
                            >{app.storageUsage.message_count} messages · {fileSize(
                                app.storageUsage.attachment_bytes + app.storageUsage.database_bytes,
                            )}<br />{app.storageUsage.attachment_count} attachments</span
                        >
                    </div>
                    <button
                        class="mt-3 inline-flex min-h-9 w-full items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                        onclick={() => requestClearHistory()}>Clear local history</button
                    >
                </section>
            {:else}
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        About TiConnect
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Version</span></div>
                        <strong>{app.bootstrap?.version ?? '0.0.6'}</strong>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Protocol</span></div>
                        <strong>ticonnect/1</strong>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Identity</span></div>
                        <strong
                            >{fingerprintLabel(app.bootstrap?.identity.fingerprint ?? '')}</strong
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Network</span></div>
                        <strong
                            >{app.users.filter((user) => user.connected).length} online · LAN only</strong
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5"><span>Licence</span></div>
                        <strong>GPL-3.0-or-later</strong>
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Discovery
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Discovery</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Find devices on the LAN</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Discovery"
                            aria-pressed={enabled('discovery.enabled', true)}
                            onclick={() =>
                                setEnabled(
                                    'discovery.enabled',
                                    !enabled('discovery.enabled', true),
                                )}
                        ></button>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">mDNS</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >_ticonnect._udp.local.</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="mDNS discovery"
                            aria-pressed={enabled('discovery.mdns', true)}
                            onclick={() =>
                                setEnabled('discovery.mdns', !enabled('discovery.mdns', true))}
                        ></button>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Broadcast fallback</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >UDP beacon on the LAN</span
                            >
                        </div>
                        <button
                            class="toggle"
                            aria-label="Broadcast discovery"
                            aria-pressed={enabled('discovery.broadcast', true)}
                            onclick={() =>
                                setEnabled(
                                    'discovery.broadcast',
                                    !enabled('discovery.broadcast', true),
                                )}
                        ></button>
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Beacon interval</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase">Seconds</span
                            >
                        </div>
                        <input
                            class="min-h-9 w-full max-w-24 shrink-0 rounded-sm border border-line bg-base px-3 py-2 text-ink placeholder:text-ink-3 hover:border-line-strong"
                            type="number"
                            min="1"
                            max="3600"
                            value={value('discovery.beacon_interval_secs', '5')}
                            oninput={(event) =>
                                setValue(
                                    'discovery.beacon_interval_secs',
                                    (event.currentTarget as HTMLInputElement).value,
                                )}
                        />
                    </div>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-4">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        LAN access
                    </h2>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Status</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >{app.network?.confidence ?? 'unknown'} confidence</span
                            >
                        </div>
                        <strong class={lanStateClass(app.network?.state ?? '')}
                            >{lanStateLabel(app.network?.state ?? '')}</strong
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Interfaces</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >Local IPv4 addresses</span
                            >
                        </div>
                        <span class="max-w-[55%] truncate text-right text-xs text-ink-2"
                            >{app.network?.interfaces || 'None'}</span
                        >
                    </div>
                    <div
                        class="flex items-center justify-between gap-6 border-b border-line/55 py-[9px] last:border-b-0"
                    >
                        <div class="flex flex-col gap-0.5">
                            <strong class="text-sm">Ports</strong><span
                                class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                                >UDP for discovery and transport</span
                            >
                        </div>
                        <span class="text-right text-xs text-ink-2"
                            >5353 · 45870 · {app.network?.transport_port ?? 45871}</span
                        >
                    </div>
                    <p class="m-0 mt-3 text-step-xs leading-[1.45] text-ink-2">
                        {app.network?.hint ?? 'No LAN status available yet.'}
                    </p>
                    <button
                        class="mt-3 inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface disabled:opacity-50"
                        disabled={app.checkingLan}
                        onclick={() => void app.checkLan()}
                        >{app.checkingLan ? 'Checking…' : 'Check LAN access'}</button
                    >
                </section>
            {/if}
        </div>
        {#if app.settingsSave === 'error'}
            <div class="shrink-0 pt-4">
                <StateBanner
                    tone="error"
                    title="Settings were not saved"
                    body={app.settingsSaveError || 'Check the daemon connection and try again.'}
                    actionLabel="Try again"
                    onaction={() => void save()}
                />
            </div>
        {/if}
        <footer class="flex shrink-0 items-center gap-2.5 pt-5">
            <span class="flex-1 text-xs text-ink-3" role="status">
                {#if saveStatus}
                    <span class={saveStatus.style}>{saveStatus.text}</span>
                {/if}
            </span>
            <button
                class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface disabled:cursor-not-allowed disabled:opacity-50"
                disabled={!dirty || app.settingsSave === 'saving'}
                onclick={revert}>Revert</button
            >
            <button
                class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2 disabled:cursor-not-allowed disabled:opacity-50"
                aria-busy={app.settingsSave === 'saving'}
                disabled={!dirty || app.settingsSave === 'saving'}
                onclick={() => void save()}
                >{app.settingsSave === 'saving' ? 'Saving…' : 'Save changes'}</button
            >
        </footer>
    </div>
    <ConfirmDialog
        open={confirmRequest !== null}
        title={confirmRequest?.title ?? ''}
        body={confirmRequest?.body ?? ''}
        confirmLabel={confirmRequest?.confirmLabel ?? 'Confirm'}
        cancelLabel={confirmRequest?.cancelLabel ?? 'Cancel'}
        tone={confirmRequest?.tone ?? 'error'}
        pending={confirmPending}
        error={confirmError}
        onconfirm={() => void runConfirm()}
        oncancel={closeConfirm}
    />
</section>

<script lang="ts">
    import type { AppState, ConfirmRequest } from '../lib';
    import { clock, fingerprintLabel, relativeSeen, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let confirmRequest = $state<ConfirmRequest | null>(null);
    let confirmPending = $state(false);
    let confirmError = $state('');
    const details = $derived(app.details);
    const user = $derived(details?.user ?? app.userById(app.selectedUserId));

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

    function requestClearHistory(): void {
        if (user === null) {
            return;
        }
        confirmError = '';
        confirmRequest = {
            title: 'Clear local history',
            body: 'Clear this conversation? This cannot be undone.',
            confirmLabel: 'Clear history',
            cancelLabel: 'Cancel',
            tone: 'error',
            action: () => app.clearHistory(user.id),
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
        if (app.party !== null && user !== null && app.party.participants.includes(user.id)) {
            return 'Watch party active';
        }
        return 'None';
    }
</script>

<section class="flex h-full min-w-0 flex-1 flex-col overflow-hidden bg-base">
    <div class="flex h-[60px] shrink-0 items-center px-7">
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
            onclick={() => app.backToChat()}
            aria-label="Back to conversation"
        >
            <Icon name="back" size={16} />
        </button>
    </div>
    {#if details && user}
        <header class="flex shrink-0 items-center gap-4 px-7 pt-0 pb-5">
            <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={64} />
            <div class="flex min-w-0 flex-1 flex-col gap-[3px]">
                <h1
                    class="m-0 font-display text-step-3 leading-[34px] font-normal tracking-[-0.01em]"
                >
                    {shortName(user)}
                </h1>
                <p
                    class="m-0 flex items-center gap-1.5 text-xs tracking-[0.06em] text-ink-2 uppercase"
                >
                    <span
                        class="size-1.5 rounded-full border border-line-strong data-online:border-success data-online:bg-success"
                        data-online={user.connected ? '' : undefined}
                    ></span>
                    {user.connected ? 'Online' : 'Offline'}
                    <span class="text-ink-3">·</span>
                    Stable identity {fingerprintLabel(user.fingerprint)}
                </p>
                <p class="m-0 text-step-xs text-ink-3">First seen {firstSeen(user.trusted_at)}</p>
            </div>
            <div class="flex shrink-0 gap-2.5">
                <button
                    class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                    onclick={() => app.navigate({ kind: 'chat', id: user.id })}
                >
                    Message
                </button>
                <button
                    class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={startCall}
                >
                    Call
                    <Icon name="call" size={15} />
                </button>
                <button
                    class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={startRemote}
                >
                    Remote
                    <Icon name="remote" size={15} />
                </button>
                <button
                    class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={() => (user ? app.openWatchParty([user.id]) : undefined)}
                >
                    Watch party
                    <Icon name="chevron-right" size={13} />
                </button>
            </div>
        </header>
        <div
            class="grid scroll min-h-0 flex-1 grid-cols-2 items-start gap-5 overflow-y-auto px-7 pt-0 pb-6"
        >
            <div class="flex flex-col gap-4">
                <section class="rounded-md border border-line bg-surface px-5 py-[18px]">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Connection
                    </h2>
                    <dl class="m-0 flex flex-col gap-0">
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Status</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink data-online:text-success"
                                data-online={user.connected ? '' : undefined}
                            >
                                {user.connected ? 'Online' : 'Offline'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Current IP</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.addresses || 'Not available'}
                            </dd>
                        </div>
                        {#if app.addressChanged}
                            <div
                                class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                            >
                                <dt class="text-step-xs text-ink-3">Previous IP</dt>
                                <dd
                                    class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-warning"
                                >
                                    {app.preferences.previousAddresses[user.id] ?? 'Unknown'}
                                </dd>
                            </div>
                            <p class="m-0 mt-1.5 text-xs tracking-[0.04em] text-warning uppercase">
                                Address changed recently
                            </p>
                        {/if}
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">MAC</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.mac || 'Not exposed'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Connected since</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {app.connectedSince[user.id]
                                    ? clock(app.connectedSince[user.id] ?? 0)
                                    : 'Unknown'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Last seen</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {relativeSeen(user.last_seen, user.connected)}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Transport</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                Encrypted · QUIC
                            </dd>
                        </div>
                    </dl>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-[18px]">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Device
                    </h2>
                    <dl class="m-0 flex flex-col gap-0">
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">OS</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.os || 'Unknown'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Display server</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.display_server || 'Unknown'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">App</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.app_version || 'Unknown'}
                            </dd>
                        </div>
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Protocol</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                ticonnect/1
                            </dd>
                        </div>
                    </dl>
                </section>
            </div>
            <div class="flex flex-col gap-4">
                <section class="rounded-md border border-line bg-surface px-5 py-[18px]">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Activity
                    </h2>
                    <dl class="m-0 flex flex-col gap-0">
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Session</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {activityText()}
                            </dd>
                        </div>
                        {#each app.callLogs
                            .filter((log) => log.user_id === user.id)
                            .slice(0, 3) as log (log.id)}
                            <div
                                class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                            >
                                <dt class="text-step-xs text-ink-3">
                                    {log.reason === 'timeout' ? 'Missed call' : 'Call'}
                                </dt>
                                <dd
                                    class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                                >
                                    {clock(log.ended_at)}
                                </dd>
                            </div>
                        {/each}
                        <div
                            class="flex items-baseline justify-between gap-4 border-b border-line/55 py-[9px] last:border-b-0"
                        >
                            <dt class="text-step-xs text-ink-3">Source</dt>
                            <dd
                                class="m-0 text-right text-step-xs [overflow-wrap:anywhere] text-ink"
                            >
                                {details.source || 'LAN'}
                            </dd>
                        </div>
                    </dl>
                </section>
                <section class="rounded-md border border-line bg-surface px-5 py-[18px]">
                    <h2
                        class="m-0 mb-3 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >
                        Policies
                    </h2>
                    <div
                        class="flex items-center justify-between gap-4 border-b border-line/55 py-2.5"
                    >
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
                    <div
                        class="flex items-center justify-between gap-4 border-b border-line/55 py-2.5"
                    >
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
                        class="mt-3 border-0 bg-transparent p-0 text-sm font-bold text-ink"
                        onclick={requestClearHistory}
                    >
                        Clear local history
                    </button>
                    <button
                        class="mt-3 inline-flex min-h-9 w-full items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                        onclick={() => void app.blockUser(user.id)}>Block user</button
                    >
                </section>
            </div>
        </div>
    {:else}
        <p class="m-0 p-8 text-step-xs text-ink-3">User details are not available.</p>
    {/if}
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

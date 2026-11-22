<script lang="ts">
    import { onMount } from 'svelte';

    import {
        CallView,
        ChatView,
        Icon,
        OverlayLayer,
        ProfileView,
        RemoteView,
        SettingsView,
        UsersRail,
        WatchPartyView,
    } from './components';
    import { AppState, matchesShortcut, parseShortcut } from './lib';

    const app = new AppState();
    const showRail = $derived(app.route.kind !== 'settings' && app.route.kind !== 'watch-party');

    onMount(() => {
        void app.start();
        return () => app.stop();
    });

    function onKeydown(event: KeyboardEvent): void {
        const target = event.target as HTMLElement | null;
        const editing =
            target?.tagName === 'INPUT' ||
            target?.tagName === 'TEXTAREA' ||
            target?.isContentEditable;
        if (event.ctrlKey && event.key === ',') {
            event.preventDefault();
            app.navigate({ kind: 'settings', section: 'user' });
            return;
        }
        if (event.ctrlKey && event.key.toLowerCase() === 'n') {
            event.preventDefault();
            app.navigate({ kind: 'chat', id: '' });
            return;
        }
        if (event.key === 'Escape' && !editing) {
            if (app.player !== null) {
                app.closePlayer();
                return;
            }
            app.dismissNotice();
            return;
        }
        if (editing || app.call === null) {
            return;
        }
        const muteShortcut = parseShortcut(app.settings['calls.mute_shortcut'] ?? 'Ctrl+M');
        if (muteShortcut !== null && matchesShortcut(event, muteShortcut)) {
            event.preventDefault();
            void app.toggleInputMute();
            return;
        }
        const outputMuteShortcut = parseShortcut(
            app.settings['calls.output_mute_shortcut'] ?? 'Ctrl+Shift+M',
        );
        if (outputMuteShortcut !== null && matchesShortcut(event, outputMuteShortcut)) {
            event.preventDefault();
            void app.toggleOutputMute();
        }
    }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-full w-full min-w-0 overflow-hidden bg-base text-ink">
    {#if showRail}
        <UsersRail {app} />
    {/if}
    <main class="relative flex h-full min-w-0 flex-1 overflow-hidden">
        {#if app.connection !== 'online'}
            <div
                class="absolute top-3 left-1/2 z-40 flex max-w-[680px] -translate-x-1/2 items-center gap-2.5 rounded-pill border border-warning bg-surface px-3.5 py-2 text-xs text-warning data-offline:border-error data-offline:text-error"
                data-offline={app.connection === 'offline' ? '' : undefined}
            >
                <span>
                    {app.connection === 'connecting'
                        ? 'Connecting to the daemon…'
                        : 'Daemon unavailable — retrying'}
                </span>
                {#if app.errorMessage}
                    <span class="truncate text-ink-3">{app.errorMessage}</span>
                {/if}
            </div>
        {/if}
        {#if app.notice}
            <div
                class="absolute top-4 left-1/2 z-[35] flex max-w-[720px] -translate-x-1/2 items-center gap-3 rounded-pill border border-line-strong bg-surface-2 py-2 pr-2.5 pl-4 text-[13px] text-ink shadow-soft"
                role="status"
            >
                <span>{app.notice}</span>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => app.dismissNotice()}
                    aria-label="Dismiss notice"
                >
                    <Icon name="close" size={13} />
                </button>
            </div>
        {/if}
        {#if app.route.kind === 'settings'}
            <SettingsView {app} />
        {:else if app.route.kind === 'watch-party'}
            <WatchPartyView {app} />
        {:else if app.route.kind === 'profile'}
            <ProfileView {app} />
        {:else if app.route.kind === 'call'}
            <CallView {app} />
        {:else if app.route.kind === 'remote'}
            <RemoteView {app} />
        {:else}
            <ChatView {app} />
        {/if}
    </main>
    <OverlayLayer {app} />
</div>

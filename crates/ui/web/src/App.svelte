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
    import { AppState } from './lib';

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
        }
    }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app-shell">
    {#if showRail}
        <UsersRail {app} />
    {/if}
    <main class="main-pane">
        {#if app.connection !== 'online'}
            <div class="connection-banner" class:offline={app.connection === 'offline'}>
                <span>
                    {app.connection === 'connecting'
                        ? 'Connecting to the daemon…'
                        : 'Daemon unavailable — retrying'}
                </span>
                {#if app.errorMessage}
                    <span class="banner-copy">{app.errorMessage}</span>
                {/if}
            </div>
        {/if}
        {#if app.notice}
            <div class="notice" role="status">
                <span>{app.notice}</span>
                <button
                    class="icon-button small"
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

<style>
    .app-shell {
        display: flex;
        width: 100%;
        height: 100%;
        min-width: 0;
        overflow: hidden;
        background: var(--base);
        color: var(--ink);
    }

    .main-pane {
        position: relative;
        display: flex;
        min-width: 0;
        flex: 1;
        height: 100%;
        overflow: hidden;
    }

    .connection-banner {
        position: absolute;
        top: 12px;
        left: 50%;
        z-index: 40;
        display: flex;
        align-items: center;
        gap: 10px;
        transform: translateX(-50%);
        padding: 8px 14px;
        border: 1px solid var(--warning);
        border-radius: var(--radius-pill);
        background: var(--surface);
        color: var(--warning);
        font-size: 12px;
        max-width: 680px;
    }

    .connection-banner.offline {
        border-color: var(--error);
        color: var(--error);
    }

    .banner-copy {
        overflow: hidden;
        color: var(--ink-3);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .notice {
        position: absolute;
        top: 16px;
        left: 50%;
        z-index: 35;
        display: flex;
        align-items: center;
        gap: 12px;
        transform: translateX(-50%);
        padding: 8px 10px 8px 16px;
        border: 1px solid var(--line-strong);
        border-radius: var(--radius-pill);
        background: var(--surface-2);
        color: var(--ink);
        box-shadow: var(--shadow-soft);
        font-size: 13px;
        max-width: 720px;
    }
</style>

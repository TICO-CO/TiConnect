<script lang="ts">
    import { onMount } from 'svelte';

    import { logoUrl } from '../assets';
    import type { AppState } from '../lib';
    import { clock, preview, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let query = $state('');
    let searchInput: HTMLInputElement | null = $state(null);

    const filteredUsers = $derived(
        app.users.filter((user) => {
            const name = shortName(user).toLowerCase();
            return name.includes(query.trim().toLowerCase());
        }),
    );

    function summaryFor(userId: string) {
        return app.summaries.find((summary) => summary.user_id === userId) ?? null;
    }

    function selectUser(userId: string) {
        app.navigate({ kind: 'chat', id: userId });
    }

    onMount(() => {
        const onKey = (event: KeyboardEvent) => {
            const target = event.target as HTMLElement | null;
            const editing =
                target?.tagName === 'INPUT' ||
                target?.tagName === 'TEXTAREA' ||
                target?.isContentEditable;
            if (event.key === '/' && !editing) {
                event.preventDefault();
                searchInput?.focus();
            }
        };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    });
</script>

<aside class="rail" class:collapsed={app.collapsed}>
    {#if app.collapsed}
        <button
            class="expand icon-button"
            onclick={() => (app.collapsed = false)}
            aria-label="Expand chats"
        >
            <Icon name="expand" />
        </button>
        <div class="monograms scroll">
            {#each filteredUsers as user (user.id)}
                <button
                    class="monogram"
                    class:selected={app.selectedUserId === user.id}
                    onclick={() => selectUser(user.id)}
                    aria-label={shortName(user)}
                    title={shortName(user)}
                >
                    <Avatar
                        {user}
                        size={36}
                        showPresence
                        selected={app.selectedUserId === user.id}
                    />
                </button>
            {/each}
        </div>
        <footer class="collapsed-footer">
            <a class="logo" href="#/chat" aria-label="TiConnect">
                <img src={logoUrl} alt="" />
            </a>
            <button
                class="icon-button small"
                onclick={() => app.navigate({ kind: 'settings', section: 'user' })}
                aria-label="Settings"
            >
                <Icon name="settings" size={15} />
            </button>
        </footer>
    {:else}
        <header class="rail-header">
            <span class="label">Chats</span>
            <button
                class="icon-button small"
                onclick={() => (app.collapsed = true)}
                aria-label="Collapse chats"
            >
                <Icon name="collapse" size={15} />
            </button>
        </header>
        <label class="search">
            <Icon name="search" size={15} color="var(--ink-3)" />
            <input
                bind:this={searchInput}
                bind:value={query}
                placeholder="Search users"
                aria-label="Search users"
            />
            <kbd>/</kbd>
        </label>
        <div class="users scroll">
            {#each filteredUsers as user (user.id)}
                {@const summary = summaryFor(user.id)}
                {@const selected = app.selectedUserId === user.id}
                {@const muted = user.policy === 'mute' || user.policy === 'ignore'}
                {@const addressChanged = summary?.last_body.includes('Address changed') ?? false}
                <button
                    class="user-card"
                    class:selected
                    onclick={() => selectUser(user.id)}
                    aria-current={selected ? 'page' : undefined}
                >
                    <Avatar {user} size={32} />
                    <span class="user-copy">
                        <span class="user-top">
                            <span class="user-name">{shortName(user)}</span>
                            <span
                                class="presence-dot"
                                class:online={user.connected}
                                aria-hidden="true"
                            ></span>
                            {#if addressChanged}
                                <span class="address-dot" title="Address changed"></span>
                            {/if}
                        </span>
                        <span class="user-preview">
                            {summary
                                ? preview(summary.last_body)
                                : muted
                                  ? 'Muted'
                                  : 'No messages yet'}
                        </span>
                    </span>
                    <span class="meta">
                        <span class="time mono-data"
                            >{summary ? clock(summary.last_sent_at) : ''}</span
                        >
                        {#if summary && summary.unread_count > 0}
                            <span class="unread mono-data">{summary.unread_count}</span>
                        {/if}
                    </span>
                </button>
            {/each}
            {#if filteredUsers.length === 0}
                <p class="empty">No users discovered yet.</p>
            {/if}
        </div>
        <footer class="rail-footer">
            <div class="footer-card">
                <img class="logo" src={logoUrl} alt="TiConnect" />
                <button
                    class="icon-button small"
                    onclick={() => app.navigate({ kind: 'settings', section: 'user' })}
                    aria-label="Settings"
                >
                    <Icon name="settings" size={16} />
                </button>
            </div>
        </footer>
    {/if}
</aside>

<style>
    .rail {
        display: flex;
        flex-direction: column;
        width: 320px;
        flex-shrink: 0;
        height: 100%;
        padding: 12px;
        gap: 6px;
        background: var(--rail);
        overflow: hidden;
    }

    .rail.collapsed {
        width: 72px;
        align-items: center;
        gap: 12px;
    }

    .rail-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        height: 30px;
        padding-inline: 12px;
    }

    .rail-header .label {
        color: var(--ink-2);
        letter-spacing: 0.1em;
    }

    .search {
        display: flex;
        align-items: center;
        gap: 8px;
        height: 36px;
        padding-inline: 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-s);
        background: var(--base);
        color: var(--ink-3);
        font-size: var(--step-xs);
    }

    .search input {
        min-width: 0;
        flex: 1;
        border: 0;
        outline: 0;
        background: transparent;
        color: var(--ink);
        font-size: var(--step-xs);
    }

    .search kbd {
        color: var(--ink-3);
        font-family: var(--font-text);
        font-size: 12px;
    }

    .users {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-height: 0;
        flex: 1;
        overflow-y: auto;
        margin-inline: -12px;
        padding-inline: 12px;
    }

    .user-card {
        display: flex;
        align-items: center;
        width: 100%;
        height: 64px;
        flex-shrink: 0;
        gap: 10px;
        padding-inline: 12px;
        border: 0;
        border-radius: var(--radius-m);
        background: transparent;
        color: var(--ink);
        text-align: left;
    }

    .user-card:hover {
        background: var(--surface);
    }

    .user-card.selected {
        width: 320px;
        margin-inline: -12px;
        padding-inline: 24px;
        border-radius: 0;
        background: var(--ink);
        color: var(--base);
    }

    .user-copy {
        display: flex;
        flex-direction: column;
        min-width: 0;
        flex: 1;
        gap: 2px;
    }

    .user-top {
        display: flex;
        align-items: center;
        gap: 6px;
        min-width: 0;
    }

    .user-name {
        overflow: hidden;
        font-size: 14px;
        font-weight: 700;
        line-height: 18px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .presence-dot {
        width: 6px;
        height: 6px;
        flex-shrink: 0;
        border-radius: 50%;
        border: 1px solid var(--line-strong);
        background: transparent;
    }

    .presence-dot.online {
        border-color: var(--lamp);
        background: var(--success);
    }

    .user-card.selected .presence-dot.online {
        border-color: var(--lamp);
        background: var(--lamp);
    }

    .address-dot {
        width: 6px;
        height: 6px;
        flex-shrink: 0;
        border-radius: 50%;
        background: var(--warning);
    }

    .user-preview {
        overflow: hidden;
        color: var(--ink-3);
        font-size: 13px;
        line-height: 18px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .user-card.selected .user-preview {
        color: var(--base);
        opacity: 0.72;
    }

    .meta {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 4px;
        width: 44px;
        flex-shrink: 0;
    }

    .time {
        color: var(--ink-3);
        font-size: 12px;
        line-height: 16px;
    }

    .user-card.selected .time {
        color: var(--base);
        opacity: 0.72;
    }

    .unread {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 20px;
        height: 18px;
        padding-inline: 6px;
        border-radius: var(--radius-pill);
        background: var(--accent);
        color: var(--base);
        font-size: 11px;
        font-weight: 700;
    }

    .rail-footer {
        margin-top: auto;
        margin-inline: -12px;
        margin-bottom: -12px;
        padding: 12px;
        border-top: 1px solid var(--line);
        background: var(--surface);
    }

    .footer-card {
        display: flex;
        align-items: center;
        justify-content: space-between;
        height: 36px;
    }

    .logo {
        display: inline-flex;
        width: 34px;
        height: 34px;
        flex-shrink: 0;
    }

    .logo img {
        width: 100%;
        height: 100%;
        object-fit: contain;
    }

    .expand {
        margin-top: 0;
    }

    .monograms {
        display: flex;
        flex-direction: column;
        align-items: center;
        width: 100%;
        gap: 10px;
        min-height: 0;
        flex: 1;
        overflow-y: auto;
    }

    .monogram {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 48px;
        height: 48px;
        flex-shrink: 0;
        border: 0;
        border-radius: var(--radius-m);
        background: transparent;
    }

    .monogram:hover {
        background: var(--surface);
    }

    .monogram.selected {
        background: var(--ink);
    }

    .collapsed-footer {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
        margin-top: auto;
        margin-inline: -12px;
        margin-bottom: -12px;
        padding: 12px 0;
        width: 72px;
        background: var(--surface);
        border-top: 1px solid var(--line);
    }
</style>

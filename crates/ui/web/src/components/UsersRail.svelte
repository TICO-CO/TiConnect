<script lang="ts">
    import { onMount } from 'svelte';

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

<aside
    class="flex h-full w-[320px] shrink-0 flex-col gap-1.5 overflow-hidden bg-rail p-3 data-collapsed:w-[72px] data-collapsed:items-center data-collapsed:gap-3"
    data-collapsed={app.collapsed ? '' : undefined}
>
    {#if app.collapsed}
        <button
            class="inline-flex size-9 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
            onclick={() => (app.collapsed = false)}
            aria-label="Expand chats"
        >
            <Icon name="expand" />
        </button>
        <div
            class="flex scroll min-h-0 w-full flex-1 flex-col items-center gap-2.5 overflow-y-auto"
        >
            {#each filteredUsers as user (user.id)}
                <button
                    class="flex size-12 shrink-0 items-center justify-center rounded-md border-0 bg-transparent hover:bg-surface data-selected:bg-ink data-selected:hover:bg-ink"
                    data-selected={app.selectedUserId === user.id ? '' : undefined}
                    onclick={() => selectUser(user.id)}
                    aria-label={shortName(user)}
                    title={shortName(user)}
                >
                    <Avatar
                        {user}
                        image={app.avatarUrl(user.fingerprint)}
                        size={36}
                        showPresence
                        selected={app.selectedUserId === user.id}
                    />
                </button>
            {/each}
        </div>
        <footer
            class="-mx-3 mt-auto -mb-3 flex w-[72px] flex-col items-center gap-2 border-t border-line bg-surface py-3"
        >
            <a class="logo-mark size-[34px] shrink-0" href="#/chat" aria-label="TiConnect"></a>
            <button
                class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
                onclick={() => app.navigate({ kind: 'settings', section: 'user' })}
                aria-label="Settings"
            >
                <Icon name="settings" size={15} />
            </button>
        </footer>
    {:else}
        <header class="flex h-[30px] items-center justify-between px-3">
            <span
                class="font-text text-xs leading-4 font-bold tracking-[0.1em] text-ink-2 uppercase"
                >Chats</span
            >
            <button
                class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                onclick={() => (app.collapsed = true)}
                aria-label="Collapse chats"
            >
                <Icon name="collapse" size={15} />
            </button>
        </header>
        <label
            class="flex h-9 items-center gap-2 rounded-sm border border-line bg-base px-3 text-step-xs text-ink-3"
        >
            <Icon name="search" size={15} color="var(--color-ink-3)" />
            <input
                class="min-w-0 flex-1 border-0 bg-transparent text-step-xs text-ink outline-none"
                bind:this={searchInput}
                bind:value={query}
                placeholder="Search users"
                aria-label="Search users"
            />
            <kbd class="font-text text-xs text-ink-3">/</kbd>
        </label>
        <div class="-mx-3 flex scroll min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-3">
            {#each filteredUsers as user (user.id)}
                {@const summary = summaryFor(user.id)}
                {@const selected = app.selectedUserId === user.id}
                {@const muted = user.policy === 'mute' || user.policy === 'ignore'}
                {@const addressChanged = summary?.last_body.includes('Address changed') ?? false}
                <button
                    class="group flex h-16 w-full shrink-0 items-center gap-2.5 rounded-md border-0 bg-transparent px-3 text-left text-ink hover:bg-surface data-selected:-mx-3 data-selected:w-[320px] data-selected:rounded-none data-selected:bg-ink data-selected:px-6 data-selected:text-base data-selected:hover:bg-ink"
                    data-selected={selected ? '' : undefined}
                    onclick={() => selectUser(user.id)}
                    aria-current={selected ? 'page' : undefined}
                >
                    <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={32} />
                    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span class="flex min-w-0 items-center gap-1.5">
                            <span class="truncate text-[14px] leading-[18px] font-bold"
                                >{shortName(user)}</span
                            >
                            <span
                                class="size-1.5 shrink-0 rounded-full border border-line-strong bg-transparent data-[presence=online]:border-lamp data-[presence=online]:bg-success data-[presence=selected-online]:border-lamp data-[presence=selected-online]:bg-lamp"
                                data-presence={selected && user.connected
                                    ? 'selected-online'
                                    : user.connected
                                      ? 'online'
                                      : undefined}
                                aria-hidden="true"
                            ></span>
                            {#if addressChanged}
                                <span
                                    class="size-1.5 shrink-0 rounded-full bg-warning"
                                    title="Address changed"
                                ></span>
                            {/if}
                        </span>
                        <span
                            class="truncate text-[13px] leading-[18px] text-ink-3 group-data-selected:text-base group-data-selected:opacity-[0.72]"
                        >
                            {summary
                                ? preview(summary.last_body)
                                : muted
                                  ? 'Muted'
                                  : 'No messages yet'}
                        </span>
                    </span>
                    <span class="flex w-11 shrink-0 flex-col items-end gap-1">
                        <span
                            class="text-xs leading-4 text-ink-3 tabular-nums group-data-selected:text-base group-data-selected:opacity-[0.72]"
                            >{summary ? clock(summary.last_sent_at) : ''}</span
                        >
                        {#if summary && summary.unread_count > 0}
                            <span
                                class="inline-flex h-[18px] min-w-5 items-center justify-center rounded-pill bg-accent px-1.5 text-[11px] font-bold text-base tabular-nums"
                                >{summary.unread_count}</span
                            >
                        {/if}
                    </span>
                </button>
            {/each}
            {#if filteredUsers.length === 0}
                <p class="m-0 text-step-xs text-ink-3">No users discovered yet.</p>
            {/if}
        </div>
        <footer class="-mx-3 mt-auto -mb-3 border-t border-line bg-surface p-3">
            <div class="flex h-9 items-center justify-between">
                <span class="logo-mark size-[34px] shrink-0" role="img" aria-label="TiConnect"
                ></span>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
                    onclick={() => app.navigate({ kind: 'settings', section: 'user' })}
                    aria-label="Settings"
                >
                    <Icon name="settings" size={16} />
                </button>
            </div>
        </footer>
    {/if}
</aside>

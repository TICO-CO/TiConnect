<script lang="ts">
    import { onMount } from 'svelte';

    import type {
        Bootstrap,
        ConversationSummary,
        Message,
        User,
        UserDetails,
        WatchParty,
        WebSocketEvent,
    } from './lib';
    import { BridgeClient, firstVideoId, resolveBridge } from './lib';

    type Connection = 'connecting' | 'online' | 'offline';
    type Route =
        | { kind: 'users' }
        | { kind: 'user'; id: string }
        | { kind: 'chat'; id: string }
        | { kind: 'settings' };

    let connection = $state<Connection>('connecting');
    let bootstrap = $state<Bootstrap | null>(null);
    let users = $state<User[]>([]);
    let summaries = $state<ConversationSummary[]>([]);
    let messages = $state<Message[]>([]);
    let details = $state<UserDetails | null>(null);
    let watchParty = $state<WatchParty | null>(null);
    let route = $state<Route>({ kind: 'users' });
    let draft = $state('');
    let errorMessage = $state('');
    let notice = $state('');
    let client: BridgeClient | null = null;

    const routeUserId = $derived.by(() => {
        if (route.kind === 'user') {
            return route.id;
        }
        if (route.kind === 'chat') {
            return route.id;
        }
        return '';
    });
    const selectedUser = $derived(users.find((user) => user.id === routeUserId) ?? null);
    const detailsUserId = $derived(details?.user.id ?? '');
    const title = $derived(
        bootstrap?.identity.display_name?.trim() || bootstrap?.identity.node_id || 'TiConnect',
    );

    onMount(() => {
        const bridge = new BridgeClient(resolveBridge(new URL(window.location.href)));
        client = bridge;
        const stopEvents = bridge.onEvent(handleEvent);
        route = readRoute();
        const onHash = () => {
            route = readRoute();
            void loadRoute();
        };
        window.addEventListener('hashchange', onHash);
        void bridge.connect().then(async ([, connectFailure]) => {
            if (connectFailure) {
                connection = 'offline';
                errorMessage = connectFailure.message;
                return;
            }
            const [bootstrapResult, bootstrapFailure] =
                await bridge.request<Bootstrap>('bootstrap');
            if (bootstrapFailure) {
                connection = 'offline';
                errorMessage = bootstrapFailure.message;
                return;
            }
            if (bootstrapResult === null) {
                connection = 'offline';
                errorMessage = 'bootstrap response was empty';
                return;
            }
            bootstrap = bootstrapResult;
            connection = 'online';
            await refresh();
            await loadRoute();
        });
        return () => {
            stopEvents();
            window.removeEventListener('hashchange', onHash);
            bridge.close();
            client = null;
        };
    });

    function readRoute(): Route {
        const path = window.location.hash.replace(/^#\/?/, '');
        const [kind, id] = path.split('/');
        if (kind === 'user' && id) {
            return { kind: 'user', id };
        }
        if (kind === 'chat' && id) {
            return { kind: 'chat', id };
        }
        if (kind === 'settings') {
            return { kind: 'settings' };
        }
        return { kind: 'users' };
    }

    function navigate(next: string) {
        window.location.hash = next;
    }

    async function refresh() {
        if (client === null) {
            return;
        }
        const [loadedUsers, usersFailure] = await client.request<User[]>('list_users');
        if (usersFailure) {
            notice = usersFailure.message;
            return;
        }
        users = loadedUsers ?? [];
        const [loadedSummaries, summariesFailure] = await client.request<ConversationSummary[]>(
            'list_conversation_summaries',
        );
        if (summariesFailure) {
            notice = summariesFailure.message;
            return;
        }
        summaries = loadedSummaries ?? [];
        const [party, partyFailure] = await client.request<WatchParty | null>('watch_party');
        if (!partyFailure) {
            watchParty = party;
        }
    }

    async function loadRoute() {
        if (route.kind === 'user') {
            await loadDetails(route.id);
            return;
        }
        if (route.kind === 'chat') {
            await loadMessages(route.id);
        }
    }

    async function loadDetails(id: string) {
        if (client === null) {
            return;
        }
        const [loaded, failure] = await client.request<UserDetails>('get_user', { user_id: id });
        if (failure) {
            notice = failure.message;
            return;
        }
        details = loaded ?? null;
    }

    async function loadMessages(id: string) {
        if (client === null) {
            return;
        }
        const summary = summaries.find((item) => item.user_id === id);
        if (summary === undefined) {
            messages = [];
            return;
        }
        const [loaded, failure] = await client.request<Message[]>('list_messages', {
            conversation_id: summary.conversation_id,
            limit: 100,
            offset: 0,
        });
        if (failure) {
            notice = failure.message;
            return;
        }
        messages = loaded ?? [];
        const unread = messages
            .filter((message) => !message.local_sender)
            .map((message) => message.id);
        if (unread.length > 0) {
            await client.request('mark_read', {
                conversation_id: summary.conversation_id,
                message_ids: unread,
            });
        }
    }

    function handleEvent(event: WebSocketEvent) {
        if (event.type === 'connected') {
            connection = 'online';
            return;
        }
        if (event.type === 'disconnected') {
            connection = 'offline';
            return;
        }
        if (event.type === 'message_received') {
            const message = event.message as Message;
            if (route.kind === 'chat' && message.user_id === route.id) {
                messages = [...messages, message];
            }
            void refresh();
            return;
        }
        if (event.type === 'watch_party_requested') {
            notice = 'YouTube watch party invite';
            return;
        }
        if (event.type === 'watch_party_changed') {
            const party = event.party as WatchParty;
            if (party.state === 'ringing') {
                return;
            }
            watchParty = party;
            return;
        }
        if (event.type === 'watch_party_ended') {
            watchParty = null;
            return;
        }
        if (event.type === 'incoming_call') {
            notice = 'Incoming voice call';
            return;
        }
        if (event.type === 'remote_session_requested') {
            notice = 'Remote desktop request';
            return;
        }
        if (
            event.type === 'user_connected' ||
            event.type === 'user_disconnected' ||
            event.type === 'peers' ||
            event.type === 'conversation_changed'
        ) {
            void refresh();
        }
    }

    async function sendMessage() {
        if (client === null || route.kind !== 'chat' || draft.trim() === '') {
            return;
        }
        const body = draft;
        draft = '';
        const [, failure] = await client.request<string>('send_message', {
            user_id: route.id,
            body,
        });
        if (failure) {
            notice = failure.message;
            return;
        }
        await loadMessages(route.id);
        await refresh();
    }

    async function setPolicy(userId: string, policy: string) {
        if (client === null) {
            return;
        }
        const [, failure] = await client.request('set_user_policy', { user_id: userId, policy });
        if (failure) {
            notice = failure.message;
            return;
        }
        await refresh();
    }

    async function forgetUser(userId: string) {
        if (client === null) {
            return;
        }
        const [, failure] = await client.request('block_user', { user_id: userId });
        if (failure) {
            notice = failure.message;
            return;
        }
        navigate('/users');
        await refresh();
    }

    async function startWatchParty(videoId: string) {
        if (client === null || route.kind !== 'chat') {
            return;
        }
        const [party, failure] = await client.request<WatchParty>('start_watch_party', {
            user_id: route.id,
            video_id: videoId,
        });
        if (failure) {
            notice = failure.message;
            return;
        }
        watchParty = party;
    }

    async function publishWatchParty(positionMs: number, playing: boolean) {
        if (client === null || watchParty === null) {
            return;
        }
        const [party, failure] = await client.request<WatchParty>('publish_watch_party', {
            party_id: watchParty.party_id,
            position_ms: positionMs,
            playing,
        });
        if (!failure) {
            watchParty = party;
        }
    }

    async function endWatchParty() {
        if (client === null || watchParty === null) {
            return;
        }
        await client.request('end_watch_party', { party_id: watchParty.party_id });
        watchParty = null;
    }

    async function saveSetting(key: string, value: string) {
        if (client === null) {
            return;
        }
        const [, failure] = await client.request('settings_save', { key, value });
        if (failure) {
            notice = failure.message;
            return;
        }
        if (bootstrap !== null) {
            bootstrap = { ...bootstrap, settings: { ...bootstrap.settings, [key]: value } };
        }
    }

    function displayName(user: User): string {
        return user.display_name.trim() || user.id;
    }

    function formatTime(value: number): string {
        if (value === 0) {
            return '';
        }
        return new Date(value).toLocaleString();
    }
</script>

<div class="app">
    <aside class="sidebar">
        <header>
            <div>
                <p class="eyebrow">TiConnect</p>
                <h1>{title}</h1>
            </div>
            <span class="status" data-state={connection}>{connection}</span>
        </header>
        <nav>
            <button class:active={route.kind === 'users'} onclick={() => navigate('/users')}
                >Users</button
            >
            <button class:active={route.kind === 'settings'} onclick={() => navigate('/settings')}>
                Settings
            </button>
        </nav>
        <div class="user-list">
            {#each users as user (user.id)}
                <button
                    class:active={route.kind === 'chat' && route.id === user.id}
                    onclick={() => navigate(`/chat/${user.id}`)}
                >
                    <span class="dot" data-online={user.connected}></span>
                    <span>{displayName(user)}</span>
                    {#if summaries.find((item) => item.user_id === user.id)?.unread_count}
                        <span class="unread">
                            {summaries.find((item) => item.user_id === user.id)?.unread_count}
                        </span>
                    {/if}
                </button>
            {/each}
            {#if users.length === 0}
                <p class="empty">No users discovered yet.</p>
            {/if}
        </div>
    </aside>

    <section class="content">
        {#if errorMessage}
            <div class="panel error">
                <h2>Bridge unavailable</h2>
                <p>{errorMessage}</p>
            </div>
        {:else if notice}
            <div class="banner">
                <span>{notice}</span>
                <button onclick={() => (notice = '')}>Dismiss</button>
            </div>
        {/if}

        {#if route.kind === 'user' && details}
            <div class="panel">
                <div class="panel-header">
                    <div>
                        <p class="eyebrow">User details</p>
                        <h2>{details.user.display_name.trim() || details.user.id}</h2>
                    </div>
                    <button onclick={() => navigate(`/chat/${details?.user.id}`)}>Message</button>
                </div>
                <dl>
                    <div>
                        <dt>Fingerprint</dt>
                        <dd class="mono">{details.user.fingerprint}</dd>
                    </div>
                    <div>
                        <dt>Node ID</dt>
                        <dd class="mono">{details.user.id}</dd>
                    </div>
                    <div>
                        <dt>Addresses</dt>
                        <dd class="mono">{details.addresses || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>Port</dt>
                        <dd>{details.port || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>MAC</dt>
                        <dd class="mono">{details.mac || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>OS</dt>
                        <dd>{details.os || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>Desktop</dt>
                        <dd>{details.desktop || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>Display server</dt>
                        <dd>{details.display_server || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>App version</dt>
                        <dd>{details.app_version || 'unknown'}</dd>
                    </div>
                    <div>
                        <dt>Last seen</dt>
                        <dd>{formatTime(details.user.last_seen)}</dd>
                    </div>
                    <div>
                        <dt>Policy</dt>
                        <dd>{details.user.policy}</dd>
                    </div>
                </dl>
                <div class="actions">
                    <button onclick={() => setPolicy(detailsUserId, 'accept')}>Accept</button>
                    <button onclick={() => setPolicy(detailsUserId, 'mute')}>Mute</button>
                    <button onclick={() => setPolicy(detailsUserId, 'ignore')}>Ignore</button>
                    <button class="danger" onclick={() => forgetUser(detailsUserId)}>Forget</button>
                </div>
            </div>
        {:else if route.kind === 'chat' && selectedUser}
            <div class="chat">
                <header class="chat-header">
                    <button class="link" onclick={() => navigate(`/user/${selectedUser.id}`)}>
                        {displayName(selectedUser)}
                    </button>
                    <span class="mono">{selectedUser.fingerprint.slice(0, 16)}…</span>
                </header>
                {#if watchParty && watchParty.user_id === selectedUser.id}
                    <div class="party">
                        <span>Watch party: {watchParty.video_id}</span>
                        <button
                            onclick={() => publishWatchParty(watchParty?.position_ms ?? 0, true)}
                        >
                            Play
                        </button>
                        <button
                            onclick={() => publishWatchParty(watchParty?.position_ms ?? 0, false)}
                        >
                            Pause
                        </button>
                        <button onclick={endWatchParty}>End</button>
                    </div>
                {/if}
                <div class="messages">
                    {#each messages as message (message.id)}
                        <article class:local={message.local_sender}>
                            <p>{message.body}</p>
                            <small>{formatTime(message.sent_at || message.received_at)}</small>
                            {#if firstVideoId(message.body)}
                                <button
                                    class="watch"
                                    onclick={() => {
                                        const videoId = firstVideoId(message.body);
                                        if (videoId !== null) {
                                            void startWatchParty(videoId);
                                        }
                                    }}
                                >
                                    Watch together
                                </button>
                            {/if}
                        </article>
                    {/each}
                    {#if messages.length === 0}
                        <p class="empty">No messages yet.</p>
                    {/if}
                </div>
                <form
                    class="composer"
                    onsubmit={(event) => {
                        event.preventDefault();
                        void sendMessage();
                    }}
                >
                    <textarea bind:value={draft} placeholder="Message" rows="2"></textarea>
                    <button type="submit">Send</button>
                </form>
            </div>
        {:else if route.kind === 'settings' && bootstrap}
            <div class="panel">
                <h2>Settings</h2>
                <label class="toggle">
                    <input
                        type="checkbox"
                        checked={bootstrap.settings['dnd.enabled'] === 'true'}
                        onchange={(event) =>
                            saveSetting(
                                'dnd.enabled',
                                String((event.currentTarget as HTMLInputElement).checked),
                            )}
                    />
                    Do not disturb
                </label>
                <label class="toggle">
                    <input
                        type="checkbox"
                        checked={bootstrap.settings['read_receipts.enabled'] !== 'false'}
                        onchange={(event) =>
                            saveSetting(
                                'read_receipts.enabled',
                                String((event.currentTarget as HTMLInputElement).checked),
                            )}
                    />
                    Send read receipts
                </label>
                <label class="toggle">
                    <input
                        type="checkbox"
                        checked={bootstrap.settings['typing.enabled'] !== 'false'}
                        onchange={(event) =>
                            saveSetting(
                                'typing.enabled',
                                String((event.currentTarget as HTMLInputElement).checked),
                            )}
                    />
                    Typing indicators
                </label>
                <p class="hint">OS, address, and display details are shown only on user pages.</p>
            </div>
        {:else}
            <div class="panel">
                <h2>Users</h2>
                <p class="hint">All users discovered on this LAN connect automatically.</p>
                <div class="user-grid">
                    {#each users as user (user.id)}
                        <button class="user-card" onclick={() => navigate(`/user/${user.id}`)}>
                            <span class="dot" data-online={user.connected}></span>
                            <strong>{displayName(user)}</strong>
                            <span class="mono">{user.fingerprint.slice(0, 16)}…</span>
                        </button>
                    {/each}
                    {#if users.length === 0}
                        <p class="empty">Waiting for users on the LAN.</p>
                    {/if}
                </div>
            </div>
        {/if}
    </section>
</div>

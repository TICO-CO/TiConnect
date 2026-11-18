import { SvelteMap } from 'svelte/reactivity';

import { BridgeClient } from './bridge';
import { demoScreen } from './demo';
import { sessionClock } from './format';
import { bridgeFromWindow, demoParameter } from './location';
import {
    DEFAULT_PREFERENCES,
    loadPreferences,
    type Preferences,
    savePreferences,
} from './preferences';
import { activationRoute, parseRoute, type Route, routeHash, type SettingsSection } from './routes';
import type {
    Bootstrap,
    Call,
    Connection,
    ConversationSummary,
    Message,
    RemoteSession,
    User,
    UserDetails,
    UserPolicy,
    WatchParty,
    WebSocketEvent,
} from './types';
import { firstVideoId } from './youtube';

export interface Toast {
    id: string;
    userId: string;
    name: string;
    body: string;
    sentAt: number;
}

export interface PlayerState {
    videoId: string;
    title: string;
    channel: string;
    positionMs: number;
    playing: boolean;
}

interface CallLog {
    id: string;
    userId: string;
    at: number;
    durationSeconds: number;
    reason: string;
}

interface DemoSeed {
    users: User[];
    bootstrap: Bootstrap;
    summaries: ConversationSummary[];
    messages: Message[];
    details: UserDetails;
    party: WatchParty | null;
    call: Call | null;
    remote: RemoteSession | null;
    route: string;
}

function nowSeconds(): number {
    return Math.floor(Date.now() / 1000);
}

function isDemoEnabled(): boolean {
    return import.meta.env.DEV && demoParameter() !== null;
}

export class AppState {
    connection = $state<Connection>('connecting');
    bootstrap = $state<Bootstrap | null>(null);
    users = $state<User[]>([]);
    summaries = $state<ConversationSummary[]>([]);
    messages = $state<Message[]>([]);
    details = $state<UserDetails | null>(null);
    call = $state<Call | null>(null);
    remote = $state<RemoteSession | null>(null);
    party = $state<WatchParty | null>(null);
    route = $state<Route>({ kind: 'chat', id: '' });
    collapsed = $state(false);
    draft = $state('');
    errorMessage = $state('');
    notice = $state('');
    toasts = $state<Toast[]>([]);
    player = $state<PlayerState | null>(null);
    typing = $state<Record<string, boolean>>({});
    callLogs = $state<CallLog[]>([]);
    addressChanged = $state(false);
    connectedSince = $state<Record<string, number>>({});
    lastChatUserId = $state('');
    selfAvatarUrl = $state('');
    avatarUrls = $state<Record<string, string>>({});
    private avatarHashes = new SvelteMap<string, string>();
    private avatarPending = new SvelteMap<string, string>();
    private lastRouteKind = '';
    preferences = $state<Preferences>({ ...DEFAULT_PREFERENCES });
    settings = $state<Record<string, string>>({});
    settingsDraft = $state<Record<string, string>>({});
    autostart = $state(false);
    autostartDraft = $state(false);
    demo = false;
    private client: BridgeClient | null = null;
    private reconnectTimer: number | null = null;
    private reconnectDelay = 1_000;
    private typingTimer: number | null = null;
    private removeHashListener: (() => void) | null = null;
    private removeBridgeListener: (() => void) | null = null;

    get selectedUserId(): string {
        if (this.route.kind === 'chat' || this.route.kind === 'profile') {
            return this.route.id;
        }
        if (this.route.kind === 'call' || this.route.kind === 'remote') {
            return this.route.id;
        }
        return '';
    }

    async start(): Promise<void> {
        if (isDemoEnabled()) {
            this.startDemo(demoParameter() ?? '');
            return;
        }
        this.preferences = loadPreferences();
        this.installHashListener();
        await this.connect();
    }

    stop(): void {
        this.removeHashListener?.();
        this.removeBridgeListener?.();
        if (this.reconnectTimer !== null) {
            window.clearTimeout(this.reconnectTimer);
        }
        if (this.typingTimer !== null) {
            window.clearTimeout(this.typingTimer);
        }
        this.client?.close();
        this.releaseAvatars();
    }

    private startDemo(screenName: string): void {
        const seed: DemoSeed = demoScreen(screenName);
        this.demo = true;
        this.connection = 'online';
        this.users = seed.users;
        this.bootstrap = seed.bootstrap;
        this.summaries = seed.summaries;
        this.messages = seed.messages;
        this.details = seed.details;
        this.party = seed.party;
        this.call = seed.call;
        this.remote = seed.remote;
        this.settings = { ...seed.bootstrap.settings };
        this.settingsDraft = { ...seed.bootstrap.settings };
        this.autostart = seed.bootstrap.autostart;
        this.autostartDraft = seed.bootstrap.autostart;
        this.connectedSince = Object.fromEntries(
            seed.users
                .filter((user) => user.connected)
                .map((user, index) => [user.id, Math.floor(Date.now() / 1_000) - 720 - index * 60]),
        );
        this.preferences = {
            ...loadPreferences(),
            userDescription: 'Quiet desk by the north window. Usually around in the evenings.',
        };
        this.route = parseRoute(window.location.hash || seed.route);
        this.installHashListener();
    }

    private installHashListener(): void {
        const listener = () => {
            this.route = parseRoute(window.location.hash);
            void this.loadRoute();
        };
        window.addEventListener('hashchange', listener);
        this.removeHashListener = () => window.removeEventListener('hashchange', listener);
    }

    private async connect(): Promise<void> {
        this.connection = this.bootstrap === null ? 'connecting' : 'online';
        const bridge = new BridgeClient(bridgeFromWindow());
        this.client = bridge;
        this.removeBridgeListener?.();
        this.removeBridgeListener = bridge.onEvent((event) => this.handleEvent(event));
        const [, connectFailure] = await bridge.connect();
        if (connectFailure) {
            this.connection = 'offline';
            this.errorMessage = connectFailure.message;
            this.scheduleReconnect();
            return;
        }
        const [bootstrap, bootstrapFailure] = await bridge.request<Bootstrap>('bootstrap');
        if (bootstrapFailure) {
            this.connection = 'offline';
            this.errorMessage = bootstrapFailure.message;
            this.scheduleReconnect();
            return;
        }
        if (bootstrap === null) {
            this.connection = 'offline';
            this.errorMessage = 'bootstrap response was empty';
            this.scheduleReconnect();
            return;
        }
        this.bootstrap = bootstrap;
        this.errorMessage = '';
        this.connection = 'online';
        this.reconnectDelay = 1_000;
        this.settings = { ...bootstrap.settings };
        this.settingsDraft = { ...bootstrap.settings };
        this.autostart = bootstrap.autostart;
        this.autostartDraft = bootstrap.autostart;
        this.route = parseRoute(window.location.hash);
        this.syncAvatars();
        await this.refresh();
        if (this.route.kind === 'chat' && this.route.id === '') {
            const first = this.summaries[0]?.user_id;
            if (first) {
                this.route = { kind: 'chat', id: first };
            }
        }
        await this.loadRoute();
    }

    private scheduleReconnect(): void {
        if (this.demo || this.reconnectTimer !== null) {
            return;
        }
        const delay = this.reconnectDelay;
        this.reconnectDelay = Math.min(this.reconnectDelay * 2, 10_000);
        this.reconnectTimer = window.setTimeout(() => {
            this.reconnectTimer = null;
            this.client?.close();
            void this.connect();
        }, delay);
    }

    private async refresh(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [loadedUsers, usersFailure] = await this.client.request<User[]>('list_users');
        if (usersFailure) {
            this.notice = usersFailure.message;
            return;
        }
        this.users = loadedUsers ?? [];
        this.syncAvatars();
        const [loadedSummaries, summariesFailure] = await this.client.request<
            ConversationSummary[]
        >('list_conversation_summaries');
        if (summariesFailure) {
            this.notice = summariesFailure.message;
            return;
        }
        this.summaries = loadedSummaries ?? [];
        const [party, partyFailure] = await this.client.request<WatchParty | null>('watch_party');
        if (!partyFailure) {
            this.party = party;
        }
    }

    avatarUrl(fingerprint: string): string {
        return this.avatarUrls[fingerprint] ?? '';
    }

    private syncAvatars(): void {
        const identity = this.bootstrap?.identity;
        this.users.forEach((user) => this.syncAvatar(user.fingerprint, user.avatar_hash));
        if (identity !== undefined) {
            this.syncAvatar(identity.fingerprint, identity.avatar_hash);
        }
        Object.keys(this.avatarUrls).forEach((fingerprint) => {
            if (fingerprint === identity?.fingerprint) {
                return;
            }
            if (this.users.some((user) => user.fingerprint === fingerprint)) {
                return;
            }
            this.clearAvatar(fingerprint);
        });
    }

    private syncAvatar(fingerprint: string, avatarHash: string): void {
        if (fingerprint === '') {
            return;
        }
        if (avatarHash === '') {
            this.clearAvatar(fingerprint);
            return;
        }
        if (this.avatarHashes.get(fingerprint) === avatarHash) {
            return;
        }
        if (this.avatarPending.get(fingerprint) === avatarHash) {
            return;
        }
        this.avatarPending.set(fingerprint, avatarHash);
        void this.fetchAvatar(fingerprint, avatarHash);
    }

    private async fetchAvatar(fingerprint: string, avatarHash: string): Promise<void> {
        const client = this.client;
        if (client === null) {
            this.avatarPending.delete(fingerprint);
            return;
        }
        const [url, failure] = await client.downloadAvatar(fingerprint);
        if (this.avatarPending.get(fingerprint) !== avatarHash) {
            if (url !== null) {
                URL.revokeObjectURL(url);
            }
            return;
        }
        this.avatarPending.delete(fingerprint);
        if (failure || url === null) {
            return;
        }
        const previousUrl = this.avatarUrls[fingerprint] ?? '';
        const previousSelfAvatarUrl = this.selfAvatarUrl;
        if (previousUrl !== '') {
            URL.revokeObjectURL(previousUrl);
        }
        this.avatarHashes.set(fingerprint, avatarHash);
        this.avatarUrls = { ...this.avatarUrls, [fingerprint]: url };
        if (this.bootstrap?.identity.fingerprint !== fingerprint) {
            return;
        }
        if (previousSelfAvatarUrl !== '' && previousSelfAvatarUrl !== previousUrl) {
            URL.revokeObjectURL(previousSelfAvatarUrl);
        }
        this.selfAvatarUrl = url;
    }

    private clearAvatar(fingerprint: string): void {
        this.avatarPending.delete(fingerprint);
        this.avatarHashes.delete(fingerprint);
        const previousUrl = this.avatarUrls[fingerprint] ?? '';
        const previousSelfAvatarUrl = this.selfAvatarUrl;
        if (this.bootstrap?.identity.fingerprint === fingerprint) {
            if (previousSelfAvatarUrl !== '' && previousSelfAvatarUrl !== previousUrl) {
                URL.revokeObjectURL(previousSelfAvatarUrl);
            }
            this.selfAvatarUrl = '';
        }
        if (previousUrl === '') {
            return;
        }
        URL.revokeObjectURL(previousUrl);
        const remaining = { ...this.avatarUrls };
        delete remaining[fingerprint];
        this.avatarUrls = remaining;
    }

    private releaseAvatars(): void {
        Object.values(this.avatarUrls).forEach((url) => URL.revokeObjectURL(url));
        if (
            this.selfAvatarUrl !== '' &&
            !Object.values(this.avatarUrls).includes(this.selfAvatarUrl)
        ) {
            URL.revokeObjectURL(this.selfAvatarUrl);
        }
        this.avatarUrls = {};
        this.avatarHashes.clear();
        this.avatarPending.clear();
        this.selfAvatarUrl = '';
    }

    private async reloadUserProfiles(): Promise<void> {
        await this.refresh();
        if (this.route.kind === 'profile') {
            await this.loadDetails(this.route.id);
        }
    }

    private async reloadSelfProfile(): Promise<void> {
        const client = this.client;
        if (client === null) {
            return;
        }
        const [bootstrap] = await client.request<Bootstrap>('bootstrap');
        if (bootstrap === null) {
            return;
        }
        this.bootstrap = bootstrap;
        this.syncAvatars();
    }

    async loadRoute(): Promise<void> {
        const previousKind = this.lastRouteKind;
        this.lastRouteKind = this.route.kind;
        if (this.route.kind === 'chat' && this.route.id) {
            this.lastChatUserId = this.route.id;
            await this.loadMessages(this.route.id);
            return;
        }
        if (this.route.kind === 'profile') {
            await this.loadDetails(this.route.id);
            return;
        }
        if (this.route.kind === 'settings') {
            if (previousKind !== 'settings') {
                await this.loadSettings();
            }
            return;
        }
        if (this.route.kind === 'call') {
            await this.loadCall();
            return;
        }
        if (this.route.kind === 'remote') {
            await this.loadRemote();
            return;
        }
        if (this.route.kind === 'watch-party') {
            await this.loadParty();
        }
    }

    async loadMessages(userId: string): Promise<void> {
        this.details = null;
        if (this.client === null) {
            return;
        }
        const summary = this.summaries.find((item) => item.user_id === userId);
        if (summary === undefined) {
            this.messages = [];
            return;
        }
        const [loaded, failure] = await this.client.request<Message[]>('list_messages', {
            conversation_id: summary.conversation_id,
            limit: 100,
            offset: 0,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        this.messages = loaded ?? [];
        const unread = this.messages
            .filter((message) => !message.local_sender)
            .map((message) => message.id);
        if (unread.length > 0) {
            await this.client.request('mark_read', {
                conversation_id: summary.conversation_id,
                message_ids: unread,
            });
        }
    }

    async loadDetails(userId: string): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [loaded, failure] = await this.client.request<UserDetails>('get_user', {
            user_id: userId,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        if (loaded !== null) {
            const previous = this.preferences.previousAddresses[userId];
            this.addressChanged =
                Boolean(previous) && Boolean(loaded.addresses) && previous !== loaded.addresses;
            if (loaded.addresses) {
                this.preferences = {
                    ...this.preferences,
                    previousAddresses: {
                        ...this.preferences.previousAddresses,
                        [userId]: previous ?? loaded.addresses,
                    },
                };
            }
        }
        this.details = loaded;
    }

    async loadSettings(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [values, failure] =
            await this.client.request<Record<string, string>>('list_settings');
        if (failure) {
            this.notice = failure.message;
            return;
        }
        this.settings = { ...(values ?? {}) };
        this.settingsDraft = { ...this.settings };
        const [enabled] = await this.client.request<boolean>('autostart_get');
        this.autostart = enabled ?? false;
        this.autostartDraft = this.autostart;
    }

    private async loadCall(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [loaded] = await this.client.request<Call | null>('call_state');
        this.call = loaded;
    }

    private async loadRemote(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [loaded] = await this.client.request<RemoteSession | null>('remote_session');
        this.remote = loaded;
    }

    private async loadParty(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const [loaded] = await this.client.request<WatchParty | null>('watch_party');
        this.party = loaded;
    }

    navigate(route: Route): void {
        this.route = route;
        const hash = routeHash(route);
        if (window.location.hash !== hash) {
            window.location.hash = hash;
            return;
        }
        void this.loadRoute();
    }

    setSection(section: SettingsSection): void {
        this.navigate({ kind: 'settings', section });
    }

    backToChat(): void {
        const id =
            this.route.kind === 'watch-party'
                ? (this.party?.user_id ?? this.lastChatUserId)
                : this.selectedUserId || this.lastChatUserId;
        this.navigate({ kind: 'chat', id });
    }

    async sendMessage(): Promise<void> {
        if (this.client === null || this.route.kind !== 'chat' || this.draft.trim() === '') {
            return;
        }
        const body = this.draft;
        const userId = this.route.id;
        this.draft = '';
        await this.setTyping(false);
        if (this.demo) {
            this.messages = [
                ...this.messages,
                {
                    id: `demo-${Date.now()}`,
                    conversation_id: 'conv-mika',
                    user_id: userId,
                    sender_id: this.bootstrap?.identity.node_id ?? '',
                    local_sender: true,
                    seq: this.messages.length + 1,
                    sent_at: Date.now(),
                    received_at: Date.now(),
                    body,
                    attachment_hash: '',
                    attachment_name: '',
                    attachment_size: 0,
                    attachment_mime: '',
                    attachment_path: '',
                    state: 'sent',
                    notify: false,
                },
            ];
            return;
        }
        const [, failure] = await this.client.request<string>('send_message', {
            user_id: userId,
            body,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadMessages(userId);
        await this.refresh();
    }

    async sendFile(file: File): Promise<void> {
        if (this.client === null || this.route.kind !== 'chat') {
            return;
        }
        const [messageId, failure] = await this.client.uploadAttachment(this.route.id, file);
        if (failure) {
            this.notice = failure.message;
            return;
        }
        if (messageId === null) {
            this.notice = 'attachment upload returned no message id';
            return;
        }
        await this.loadMessages(this.route.id);
        await this.refresh();
    }

    async uploadAvatar(file: File): Promise<void> {
        this.selfAvatarUrl = URL.createObjectURL(file);
        if (this.client === null || this.demo) {
            this.notice = 'Avatar preview updated for this demo.';
            return;
        }
        const [, failure] = await this.client.uploadAvatar(file);
        if (failure) {
            this.notice = failure.message;
            return;
        }
        this.notice = 'Avatar updated.';
        const [bootstrap] = await this.client.request<Bootstrap>('bootstrap');
        if (bootstrap !== null) {
            this.bootstrap = bootstrap;
            this.syncAvatars();
        }
    }

    async openAttachment(hash: string): Promise<void> {
        if (this.client === null || hash === '') {
            this.notice = 'This attachment is not available in the demo.';
            return;
        }
        const [url, failure] = await this.client.downloadAttachment(hash);
        if (failure) {
            this.notice = failure.message;
            return;
        }
        if (url !== null) {
            window.open(url, '_blank', 'noopener');
        }
    }

    updateDraft(value: string): void {
        this.draft = value;
        if (this.client === null || this.route.kind !== 'chat') {
            return;
        }
        void this.setTyping(value.trim() !== '');
        if (this.typingTimer !== null) {
            window.clearTimeout(this.typingTimer);
        }
        this.typingTimer = window.setTimeout(() => void this.setTyping(false), 2_500);
    }

    private async setTyping(active: boolean): Promise<void> {
        if (this.client === null || this.route.kind !== 'chat') {
            return;
        }
        await this.client.request('set_typing', { user_id: this.route.id, typing: active });
    }

    async updatePolicy(userId: string, policy: UserPolicy): Promise<void> {
        if (this.client === null) {
            this.users = this.users.map((user) =>
                user.id === userId ? { ...user, policy } : user,
            );
            return;
        }
        const [, failure] = await this.client.request('set_user_policy', {
            user_id: userId,
            policy,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.refresh();
        if (this.details?.user.id === userId) {
            await this.loadDetails(userId);
        }
    }

    async blockUser(userId: string): Promise<void> {
        if (this.client !== null) {
            const [, failure] = await this.client.request('block_user', { user_id: userId });
            if (failure) {
                this.notice = failure.message;
                return;
            }
        }
        this.navigate({ kind: 'chat', id: '' });
        await this.refresh();
    }

    async startCall(userId: string): Promise<void> {
        if (this.client === null) {
            this.call = {
                id: 'demo-call',
                user_id: userId,
                direction: 'outgoing',
                state: 'ringing',
                started_at: nowSeconds(),
                connected_at: 0,
            };
            this.navigate({ kind: 'call', id: userId });
            return;
        }
        const [, failure] = await this.client.request<string>('start_call', { user_id: userId });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadCall();
        this.navigate({ kind: 'call', id: userId });
    }

    async acceptCall(): Promise<void> {
        if (this.client === null || this.call === null) {
            return;
        }
        const [, failure] = await this.client.request('accept_call', { call_id: this.call.id });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadCall();
        this.navigate({ kind: 'call', id: this.call.user_id });
    }

    async declineCall(): Promise<void> {
        if (this.client === null || this.call === null) {
            this.call = null;
            return;
        }
        await this.client.request('decline_call', { call_id: this.call.id });
        this.call = null;
    }

    async hangupCall(): Promise<void> {
        if (this.client === null || this.call === null) {
            this.call = null;
            return;
        }
        await this.client.request('hangup_call', { call_id: this.call.id });
        this.call = null;
    }

    async requestRemote(userId: string): Promise<void> {
        if (this.client === null) {
            this.remote = {
                session_id: 'demo-remote',
                user_id: userId,
                fingerprint: '',
                role: 'viewer',
                state: 'requested',
                indicator: false,
                expires_at: nowSeconds() + 300,
            };
            this.navigate({ kind: 'remote', id: userId });
            return;
        }
        const [, failure] = await this.client.request<RemoteSession>('request_remote_session', {
            user_id: userId,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadRemote();
        this.navigate({ kind: 'remote', id: userId });
    }

    async acceptRemote(): Promise<void> {
        if (this.client === null || this.remote === null) {
            if (this.remote !== null) {
                this.remote = { ...this.remote, state: 'active', indicator: true };
            }
            return;
        }
        const [, failure] = await this.client.request('accept_remote_session', {
            session_id: this.remote.session_id,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadRemote();
    }

    async denyRemote(): Promise<void> {
        if (this.client === null || this.remote === null) {
            this.remote = null;
            return;
        }
        await this.client.request('deny_remote_session', {
            session_id: this.remote.session_id,
        });
        this.remote = null;
    }

    async endRemote(): Promise<void> {
        if (this.client === null || this.remote === null) {
            this.remote = null;
            return;
        }
        await this.client.request('end_remote_session', {
            session_id: this.remote.session_id,
        });
        this.remote = null;
    }

    async startParty(videoId: string, userId: string): Promise<void> {
        if (this.client === null) {
            this.party = {
                party_id: 'demo-party',
                user_id: userId,
                video_id: videoId,
                role: 'host',
                state: 'active',
                position_ms: 0,
                playing: true,
                revision: 1,
                actor: this.bootstrap?.identity.node_id ?? '',
                updated_at: nowSeconds(),
            };
            this.openPlayer(videoId, 0, true);
            this.navigate({ kind: 'watch-party' });
            return;
        }
        const [party, failure] = await this.client.request<WatchParty>('start_watch_party', {
            user_id: userId,
            video_id: videoId,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        this.party = party;
        this.navigate({ kind: 'watch-party' });
    }

    async acceptParty(): Promise<void> {
        if (this.client === null || this.party === null) {
            if (this.party !== null) {
                this.party = { ...this.party, state: 'active' };
            }
            return;
        }
        const [, failure] = await this.client.request('accept_watch_party', {
            party_id: this.party.party_id,
        });
        if (failure) {
            this.notice = failure.message;
            return;
        }
        await this.loadParty();
        this.navigate({ kind: 'watch-party' });
    }

    async declineParty(): Promise<void> {
        if (this.client === null || this.party === null) {
            this.party = null;
            return;
        }
        await this.client.request('decline_watch_party', { party_id: this.party.party_id });
        this.party = null;
    }

    async publishParty(positionMs: number, playing: boolean): Promise<void> {
        if (this.client === null || this.party === null) {
            if (this.party !== null) {
                this.party = { ...this.party, position_ms: positionMs, playing };
            }
            return;
        }
        const [party, failure] = await this.client.request<WatchParty>('publish_watch_party', {
            party_id: this.party.party_id,
            position_ms: positionMs,
            playing,
        });
        if (!failure && party !== null) {
            this.party = party;
        }
    }

    async endParty(): Promise<void> {
        if (this.client !== null && this.party !== null) {
            await this.client.request('end_watch_party', { party_id: this.party.party_id });
        }
        this.party = null;
        this.closePlayer();
        this.backToChat();
    }

    setDraftSetting(key: string, value: string): void {
        this.settingsDraft = { ...this.settingsDraft, [key]: value };
    }

    setAutostart(value: boolean): void {
        this.autostartDraft = value;
    }

    updatePreferences(patch: Partial<Preferences>): void {
        this.preferences = { ...this.preferences, ...patch };
        savePreferences(this.preferences);
    }

    async saveSettings(): Promise<void> {
        if (this.client === null) {
            this.settings = { ...this.settingsDraft };
            this.autostart = this.autostartDraft;
            this.notice = 'Settings saved for this demo.';
            return;
        }
        for (const [key, value] of Object.entries(this.settingsDraft)) {
            if (this.settings[key] === value) {
                continue;
            }
            const [, failure] = await this.client.request('settings_save', { key, value });
            if (failure) {
                this.notice = failure.message;
                return;
            }
        }
        if (this.autostart !== this.autostartDraft) {
            const [, failure] = await this.client.request('autostart_set', {
                enabled: this.autostartDraft,
            });
            if (failure) {
                this.notice = failure.message;
                return;
            }
        }
        this.notice = 'Settings saved.';
        await this.loadSettings();
        this.bootstrap =
            this.bootstrap === null ? null : { ...this.bootstrap, autostart: this.autostart };
    }

    revertSettings(): void {
        this.settingsDraft = { ...this.settings };
        this.autostartDraft = this.autostart;
    }

    openPlayer(videoId: string, positionMs = 0, playing = true): void {
        this.player = {
            videoId,
            title: 'How we built TiConnect for the LAN',
            channel: 'TiConnect Labs',
            positionMs,
            playing,
        };
    }

    closePlayer(): void {
        this.player = null;
    }

    toggleIgnoreIncoming(): void {
        this.updatePreferences({ ignoreIncoming: !this.preferences.ignoreIncoming });
    }

    dismissToast(id: string): void {
        this.toasts = this.toasts.filter((toast) => toast.id !== id);
    }

    dismissNotice(): void {
        this.notice = '';
    }

    private handleEvent(event: WebSocketEvent): void {
        switch (event.type) {
            case 'connected':
                this.connection = 'online';
                this.errorMessage = '';
                void this.refresh();
                break;
            case 'disconnected':
                this.connection = 'offline';
                this.scheduleReconnect();
                break;
            case 'activate': {
                const view = typeof event.view === 'string' ? event.view : 'chat';
                const userId = typeof event.user_id === 'string' ? event.user_id : undefined;
                this.navigate(activationRoute(view, userId));
                break;
            }
            case 'message_received':
                this.receiveMessage(event.message as Message);
                break;
            case 'message_state_changed': {
                const messageId = String(event.message_id ?? '');
                const state = String(event.state ?? '');
                this.messages = this.messages.map((message) =>
                    message.id === messageId ? { ...message, state } : message,
                );
                break;
            }
            case 'typing_changed': {
                const userId = String(event.user_id ?? '');
                this.typing = { ...this.typing, [userId]: Boolean(event.typing) };
                break;
            }
            case 'user_profile_changed':
                void this.reloadUserProfiles();
                break;
            case 'self_profile_changed':
                void this.reloadSelfProfile();
                break;
            case 'conversation_changed':
            case 'user_connected':
            case 'user_disconnected':
            case 'users':
            case 'policy_changed':
                void this.refresh();
                break;
            case 'incoming_call':
                this.call = event.call as Call;
                break;
            case 'call_state_changed':
                if (this.call !== null && this.call.id === String(event.call_id ?? '')) {
                    this.call = { ...this.call, state: String(event.state ?? this.call.state) };
                }
                break;
            case 'call_ended': {
                this.finishCall(String(event.call_id ?? ''), String(event.reason ?? ''));
                break;
            }
            case 'remote_session_requested':
            case 'remote_session_started':
                this.remote = (event.session as RemoteSession | undefined) ?? this.remote;
                if (this.remote !== null) {
                    this.remote = { ...this.remote, state: 'active', indicator: true };
                }
                break;
            case 'remote_session_ended':
                this.remote = null;
                break;
            case 'remote_indicator_changed':
                if (this.remote !== null) {
                    this.remote = { ...this.remote, indicator: Boolean(event.visible) };
                }
                break;
            case 'watch_party_requested':
            case 'watch_party_changed':
                this.party = event.party as WatchParty;
                break;
            case 'watch_party_ended':
                this.party = null;
                this.closePlayer();
                break;
            case 'settings_changed':
                void this.loadSettings();
                break;
            case 'error':
                this.notice = String(event.message ?? 'bridge error');
                break;
            default:
                break;
        }
    }

    private receiveMessage(message: Message): void {
        if (this.route.kind === 'chat' && this.route.id === message.user_id) {
            this.messages = [...this.messages.filter((item) => item.id !== message.id), message];
        }
        const videoId = firstVideoId(message.body);
        if (
            videoId !== null &&
            this.preferences.autoplay &&
            !this.preferences.ignoreIncoming &&
            this.party === null
        ) {
            this.openPlayer(videoId, 0, true);
        }
        if (this.route.kind !== 'chat' || this.route.id !== message.user_id) {
            if (message.local_sender) {
                void this.refresh();
                return;
            }
            this.toasts = [
                ...this.toasts.filter((toast) => toast.id !== message.id),
                {
                    id: message.id,
                    userId: message.user_id,
                    name: this.userName(message.user_id),
                    body: message.body,
                    sentAt: message.sent_at || message.received_at,
                },
            ].slice(-3);
        }
        void this.refresh();
    }

    private finishCall(callId: string, reason: string): void {
        const finished = this.call;
        this.call = null;
        if (finished === null) {
            return;
        }
        const duration = Math.max(0, Math.floor((Date.now() - finished.started_at) / 1_000));
        this.callLogs = [
            ...this.callLogs,
            {
                id: callId,
                userId: finished.user_id,
                at: nowSeconds(),
                durationSeconds: duration,
                reason,
            },
        ];
        this.toasts = [
            ...this.toasts,
            {
                id: `call-${callId}`,
                userId: finished.user_id,
                name: this.userName(finished.user_id),
                body: `Voice call ended · ${sessionClock(duration)}`,
                sentAt: nowSeconds(),
            },
        ].slice(-3);
    }

    userName(userId: string): string {
        const user = this.users.find((item) => item.id === userId);
        if (user === undefined) {
            return userId.slice(0, 8);
        }
        return user.display_name.trim() || user.id.slice(0, 8);
    }

    userById(userId: string): User | null {
        return this.users.find((user) => user.id === userId) ?? null;
    }
}

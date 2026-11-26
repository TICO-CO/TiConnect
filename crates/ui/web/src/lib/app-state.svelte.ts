import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import { BridgeClient } from './bridge';
import { demoScreen } from './demo';
import { sessionClock } from './format';
import { bridgeFromWindow, demoParameter } from './location';
import {
    clampVolume,
    DEFAULT_PREFERENCES,
    DEFAULT_USER_VOLUME,
    loadPreferences,
    MAX_VOLUME,
    type Preferences,
    savePreferences,
} from './preferences';
import { attempt, attemptAsync } from './result';
import { activationRoute, parseRoute, type Route, routeHash, type SettingsSection } from './routes';
import type {
    AudioDevice,
    Bootstrap,
    Call,
    CallLog,
    CallMetrics,
    Connection,
    ConversationSummary,
    Message,
    NetworkStatus,
    RemoteSession,
    StorageUsage,
    User,
    UserDetails,
    UserPolicy,
    WatchParty,
    WebSocketEvent,
    YoutubePreview,
    YoutubeStream,
} from './types';
import type { LifecycleState, LoadState, PartyOutcome, RemoteOutcome, SaveState } from './ui-state';
import { pushWaveSample } from './waveform';
import { firstVideoId } from './youtube';

function timing(label: string): void {
    if (import.meta.env.DEV) {
        console.info(`[ui-timing] ${performance.now().toFixed(0)}ms ${label}`);
    }
}

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

export interface TransferState {
    bytes: number;
    total: number;
    done: boolean;
    failed: string;
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

const HISTORY_PAGE_SIZE = 100;
const LIVE_SETTING_SAVE_DELAY_MS = 80;
const RECORDING_LEVEL_SAMPLE_MS = 100;
const RECORDING_RMS_GAIN = 4;
const RECORDING_LEVEL_ATTACK = 0.6;
const RECORDING_LEVEL_RELEASE = 0.25;
const MAX_MESSAGE_BODY_BYTES = 32 * 1024;
const TOAST_LIMIT = 3;
const DEFAULT_TOAST_TIMEOUT_SECS = 5;

function nowSeconds(): number {
    return Math.floor(Date.now() / 1000);
}

function messageBodyBytes(value: string): number {
    return new TextEncoder().encode(value).length;
}

function isDemoEnabled(): boolean {
    return import.meta.env.DEV && demoParameter() !== null;
}

export class AppState {
    connection = $state<Connection>('connecting');
    lifecycle = $state<LifecycleState>('booting');
    reconnectAttempt = $state(0);
    bootstrap = $state<Bootstrap | null>(null);
    network = $state<NetworkStatus | null>(null);
    checkingLan = $state(false);
    users = $state<User[]>([]);
    usersLoad = $state<LoadState>('idle');
    usersError = $state('');
    summaries = $state<ConversationSummary[]>([]);
    messages = $state<Message[]>([]);
    messagesLoad = $state<LoadState>('idle');
    messagesError = $state('');
    olderMessagesError = $state('');
    hasOlderMessages = $state(false);
    loadingOlderMessages = $state(false);
    details = $state<UserDetails | null>(null);
    call = $state<Call | null>(null);
    remote = $state<RemoteSession | null>(null);
    remoteOutcome = $state<RemoteOutcome | null>(null);
    party = $state<WatchParty | null>(null);
    partyOutcome = $state<PartyOutcome | null>(null);
    watchPartySelection = $state<string[]>([]);
    route = $state<Route>({ kind: 'chat', id: '' });
    collapsed = $state(false);
    draft = $state('');
    errorMessage = $state('');
    messageRetryPending = $state('');
    messageActionPending = $state('');
    replyToId = $state('');
    notice = $state('');
    toasts = $state<Toast[]>([]);
    transfers = $state<Record<string, TransferState>>({});
    player = $state<PlayerState | null>(null);
    typing = $state<Record<string, boolean>>({});
    callLogs = $state<CallLog[]>([]);
    audioDevices = $state<AudioDevice[]>([]);
    addressChanged = $state(false);
    connectedSince = $state<Record<string, number>>({});
    lastChatUserId = $state('');
    selfAvatarUrl = $state('');
    avatarUrls = $state<Record<string, string>>({});
    private avatarHashes = new SvelteMap<string, string>();
    private avatarPending = new SvelteMap<string, string>();
    private attachmentUrls = new SvelteMap<string, string>();
    private attachmentPending = new SvelteMap<string, Promise<string>>();
    private lastRouteKind = '';
    preferences = $state<Preferences>({ ...DEFAULT_PREFERENCES });
    settings = $state<Record<string, string>>({});
    settingsDraft = $state<Record<string, string>>({});
    settingsLoad = $state<LoadState>('idle');
    settingsLoadError = $state('');
    settingsSave = $state<SaveState>('idle');
    settingsSaveError = $state('');
    autostart = $state(false);
    autostartDraft = $state(false);
    storageUsage = $state<StorageUsage>({
        message_count: 0,
        attachment_count: 0,
        attachment_bytes: 0,
        database_bytes: 0,
    });
    recording = $state(false);
    recordingSeconds = $state(0);
    recordingLevel = $state(0);
    recordingWaveform = $state<number[]>([]);
    callLevel = $state(0);
    callActionPending = $state(false);
    remoteInputPending = $state(false);
    demo = false;
    private client: BridgeClient | null = null;
    private videoPreviews = new SvelteMap<string, YoutubePreview>();
    private videoStreamPending = new SvelteMap<string, Promise<string | null>>();
    private preparedVideos = new SvelteSet<string>();
    private prefetchAttempts = new SvelteSet<string>();
    preparingVideos = new SvelteSet<string>();
    private reconnectTimer: number | null = null;
    private reconnectDelay = 1_000;
    private refreshTimer: number | null = null;
    private callLevelTimer: number | null = null;
    private callLevelPending = false;
    private typingTimer: number | null = null;
    private liveSettingTimers = new Map<string, number>();
    private pendingLiveSettings = new Map<string, string>();
    private toastTimers = new SvelteMap<string, number>();
    private messageOffset = 0;
    private readMessageIds = new SvelteSet<string>();
    private removeHashListener: (() => void) | null = null;
    private removeBridgeListener: (() => void) | null = null;
    private appearanceMedia: MediaQueryList | null = null;
    private appearanceMediaListener: (() => void) | null = null;
    private recorder: MediaRecorder | null = null;
    private recordingChunks: Blob[] = [];
    private recordingStream: MediaStream | null = null;
    private recordingTimer: number | null = null;
    private recordingContext: AudioContext | null = null;
    private recordingSource: MediaStreamAudioSourceNode | null = null;
    private recordingLevelTimer: number | null = null;
    private discardRecording = false;

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
        timing('frontend start');
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
        this.removeAppearanceMediaListener();
        if (this.reconnectTimer !== null) {
            window.clearTimeout(this.reconnectTimer);
        }
        if (this.refreshTimer !== null) {
            window.clearTimeout(this.refreshTimer);
        }
        this.stopCallLevelPolling();
        this.callLevel = 0;
        if (this.typingTimer !== null) {
            window.clearTimeout(this.typingTimer);
        }
        for (const timer of this.toastTimers.values()) {
            window.clearTimeout(timer);
        }
        this.toastTimers.clear();
        this.cancelLiveSettings();
        this.cancelRecording();
        this.client?.close();
        this.releaseAvatars();
        this.releaseAttachments();
    }

    async retryConnection(): Promise<void> {
        if (this.demo) {
            return;
        }
        if (this.reconnectTimer !== null) {
            window.clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        this.reconnectDelay = 1_000;
        this.client?.close();
        await this.connect();
    }

    async retryRefresh(): Promise<void> {
        await this.refresh();
    }

    private startDemo(screenName: string): void {
        const seed: DemoSeed = demoScreen(screenName);
        this.demo = true;
        this.connection = 'online';
        this.lifecycle = 'ready';
        this.users = seed.users;
        this.usersLoad = seed.users.length === 0 ? 'empty' : 'ready';
        this.usersError = '';
        this.bootstrap = seed.bootstrap;
        this.network = seed.bootstrap.network;
        this.summaries = seed.summaries;
        this.messages = seed.messages;
        this.messagesLoad = seed.messages.length === 0 ? 'empty' : 'ready';
        this.messagesError = '';
        this.olderMessagesError = '';
        this.details = seed.details;
        this.party = seed.party;
        this.call = seed.call;
        this.remote = seed.remote;
        this.settings = { ...seed.bootstrap.settings };
        this.settingsDraft = { ...seed.bootstrap.settings };
        this.autostart = seed.bootstrap.autostart;
        this.autostartDraft = seed.bootstrap.autostart;
        this.storageUsage = {
            message_count: 128,
            attachment_count: 7,
            attachment_bytes: 18_400_000,
            database_bytes: 2_900_000,
        };
        this.applyAppearance();
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
        this.connection = this.bootstrap === null ? 'connecting' : 'offline';
        this.lifecycle = this.bootstrap === null ? 'booting' : 'ready';
        const bridge = new BridgeClient(bridgeFromWindow());
        this.client = bridge;
        this.removeBridgeListener?.();
        this.removeBridgeListener = bridge.onEvent((event) => this.handleEvent(event));
        const connectionResult = await bridge.connect();
        if (!connectionResult.ok) {
            this.connection = 'offline';
            this.lifecycle = this.bootstrap === null ? 'offline' : 'ready';
            this.errorMessage = connectionResult.error.message;
            this.scheduleReconnect();
            return;
        }
        timing('bridge connected');
        const bootstrapResult = await bridge.request<Bootstrap>('bootstrap');
        if (!bootstrapResult.ok) {
            this.connection = 'offline';
            this.lifecycle = this.bootstrap === null ? 'offline' : 'ready';
            this.errorMessage = bootstrapResult.error.message;
            this.scheduleReconnect();
            return;
        }
        const bootstrap = bootstrapResult.value;
        if (bootstrap === null) {
            this.connection = 'offline';
            this.lifecycle = this.bootstrap === null ? 'offline' : 'ready';
            this.errorMessage = 'bootstrap response was empty';
            this.scheduleReconnect();
            return;
        }
        this.bootstrap = bootstrap;
        this.network = bootstrap.network;
        this.errorMessage = '';
        this.connection = 'online';
        this.lifecycle = 'ready';
        this.reconnectAttempt = 0;
        timing('bootstrap loaded');
        this.reconnectDelay = 1_000;
        this.settings = { ...bootstrap.settings };
        this.settingsDraft = { ...bootstrap.settings };
        this.autostart = bootstrap.autostart;
        this.autostartDraft = bootstrap.autostart;
        this.applyAppearance();
        this.route = parseRoute(window.location.hash);
        this.syncAvatars();
        await this.refresh();
        await this.loadCallLogs();
        if (this.route.kind === 'chat' && this.route.id === '') {
            const first = this.summaries[0]?.user_id;
            if (first) {
                this.route = { kind: 'chat', id: first };
            }
        }
        await this.loadRoute();
        timing('route loaded');
    }

    private scheduleReconnect(): void {
        if (this.demo || this.reconnectTimer !== null) {
            return;
        }
        this.reconnectAttempt += 1;
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
        this.usersLoad = 'loading';
        this.usersError = '';
        const usersResult = await this.client.request<User[]>('list_users');
        if (!usersResult.ok) {
            this.usersLoad = 'error';
            this.usersError = usersResult.error.message;
            this.notice = usersResult.error.message;
            return;
        }
        this.users = usersResult.value ?? [];
        this.usersLoad = this.users.length === 0 ? 'empty' : 'ready';
        this.syncAvatars();
        const summariesResult = await this.client.request<ConversationSummary[]>(
            'list_conversation_summaries',
        );
        if (!summariesResult.ok) {
            this.notice = summariesResult.error.message;
            return;
        }
        this.summaries = summariesResult.value ?? [];
        await this.refreshNetwork();
        const partyResult = await this.client.request<WatchParty | null>('watch_party');
        if (partyResult.ok) {
            this.adoptParty(partyResult.value);
        }
    }

    get networkWarning(): string {
        switch (this.network?.state) {
            case 'no_interface':
                return 'No LAN connection detected. Connect to Wi-Fi or Ethernet.';
            case 'bind_failed':
                return 'TiConnect cannot open its network port. Check for a port conflict.';
            case 'discovery_down':
                return 'LAN discovery could not start. Check sandbox permissions and port conflicts.';
            case 'peers_unreachable':
                return 'Peers were found but cannot connect. Check the firewall and network isolation.';
            case undefined:
            default:
                return '';
        }
    }

    get settingsDirty(): boolean {
        if (this.autostart !== this.autostartDraft) {
            return true;
        }
        return Object.entries(this.settingsDraft).some(
            ([key, value]) => this.settings[key] !== value,
        );
    }

    async checkLan(): Promise<void> {
        if (this.client === null || this.checkingLan) {
            return;
        }
        this.checkingLan = true;
        const result = await this.client.request<NetworkStatus>('run_lan_check');
        this.checkingLan = false;
        if (!result.ok) {
            this.notice = result.error.message;
            return;
        }
        if (result.value !== null) {
            this.network = result.value;
            this.notice = 'LAN check complete.';
        }
    }

    private async refreshNetwork(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const result = await this.client.request<NetworkStatus>('network_status');
        if (result.ok && result.value !== null) {
            this.network = result.value;
        }
    }

    private scheduleRefresh(): void {
        if (this.demo) {
            return;
        }
        if (this.refreshTimer !== null) {
            window.clearTimeout(this.refreshTimer);
        }
        this.refreshTimer = window.setTimeout(() => {
            this.refreshTimer = null;
            void this.refresh();
        }, 500);
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
        const avatarResult = await client.downloadAvatar(fingerprint);
        if (this.avatarPending.get(fingerprint) !== avatarHash) {
            if (avatarResult.ok) {
                URL.revokeObjectURL(avatarResult.value);
            }
            return;
        }
        this.avatarPending.delete(fingerprint);
        if (!avatarResult.ok) {
            return;
        }
        const url = avatarResult.value;
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

    private releaseAttachments(): void {
        this.attachmentUrls.forEach((url) => URL.revokeObjectURL(url));
        this.attachmentUrls.clear();
        this.attachmentPending.clear();
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
        const bootstrapResult = await client.request<Bootstrap>('bootstrap');
        if (!bootstrapResult.ok) {
            return;
        }
        const bootstrap = bootstrapResult.value;
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
            this.applyAppearance();
            if (this.route.section === 'storage') {
                await this.loadStorageUsage();
            }
            if (this.route.section === 'calls' || this.route.section === 'audio') {
                await this.loadAudioDevices();
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

    private normalizeMessage(message: Message): Message {
        const nodeId = this.bootstrap?.identity.node_id;
        if (nodeId === undefined) {
            return message;
        }
        return { ...message, local_sender: message.sender_id === nodeId };
    }

    private applyMessageState(messageId: string, state: string): void {
        const index = this.messages.findIndex((message) => message.id === messageId);
        const message = this.messages[index];
        if (message === undefined) {
            return;
        }
        const next = this.messages.slice();
        next[index] = { ...message, state };
        this.messages = next;
    }

    private appendMessage(message: Message): void {
        const normalized = this.normalizeMessage(message);
        const index = this.messages.findIndex((item) => item.id === normalized.id);
        if (index >= 0) {
            const next = this.messages.slice();
            next[index] = normalized;
            this.messages = next;
            return;
        }
        this.messages = [...this.messages, normalized];
    }

    private updateSummary(message: Message): void {
        const userId = message.user_id || this.selectedUserId;
        if (userId === '') {
            return;
        }
        const index = this.summaries.findIndex((item) => item.user_id === userId);
        const current = this.summaries[index];
        if (current === undefined) {
            return;
        }
        const updated: ConversationSummary = {
            ...current,
            last_message_id: message.id,
            last_sender_id: message.sender_id,
            last_body: message.body,
            last_sent_at: message.sent_at || message.received_at,
        };
        this.summaries = this.summaries.map((item, itemIndex) =>
            itemIndex === index ? updated : item,
        );
    }

    private async loadSentMessage(messageId: string, userId: string): Promise<void> {
        const client = this.client;
        if (client === null) {
            return;
        }
        const messageResult = await client.request<Message>('get_message', {
            message_id: messageId,
        });
        if (!messageResult.ok) {
            await this.loadMessages(userId);
            return;
        }
        const message = messageResult.value;
        if (message === null) {
            await this.loadMessages(userId);
            return;
        }
        if (message.user_id === '') {
            message.user_id = userId;
        }
        if (this.route.kind === 'chat' && this.route.id === userId) {
            this.appendMessage(message);
        }
        this.updateSummary(message);
    }

    async loadMessages(userId: string): Promise<void> {
        this.details = null;
        if (this.client === null) {
            return;
        }
        this.messagesLoad = 'loading';
        this.messagesError = '';
        this.olderMessagesError = '';
        this.messages = [];
        this.messageOffset = 0;
        this.hasOlderMessages = false;
        this.loadingOlderMessages = false;
        const summary = this.summaries.find((item) => item.user_id === userId);
        if (summary === undefined) {
            this.messagesLoad = 'empty';
            return;
        }
        const conversationId = summary.conversation_id;
        const countResult = await this.client.request<number>('count_messages', {
            conversation_id: conversationId,
        });
        if (!countResult.ok) {
            this.messagesLoad = 'error';
            this.messagesError = countResult.error.message;
            this.notice = countResult.error.message;
            return;
        }
        const messageCount = countResult.value ?? 0;
        if (messageCount > 0) {
            const limit = Math.min(HISTORY_PAGE_SIZE, messageCount);
            const offset = messageCount - limit;
            const messagesResult = await this.client.request<Message[]>('list_messages', {
                conversation_id: conversationId,
                limit,
                offset,
            });
            if (!messagesResult.ok) {
                this.messagesLoad = 'error';
                this.messagesError = messagesResult.error.message;
                this.notice = messagesResult.error.message;
                return;
            }
            if (this.route.kind !== 'chat' || this.route.id !== userId) {
                return;
            }
            this.messages = (messagesResult.value ?? []).map((message) =>
                this.normalizeMessage(message),
            );
            this.messageOffset = offset;
            this.hasOlderMessages = offset > 0;
        }
        if (this.route.kind !== 'chat' || this.route.id !== userId) {
            return;
        }
        this.messagesLoad = this.messages.length === 0 ? 'empty' : 'ready';
        await this.markConversationRead(conversationId);
    }

    async loadOlderMessages(): Promise<void> {
        if (
            this.client === null ||
            this.route.kind !== 'chat' ||
            this.loadingOlderMessages ||
            this.messageOffset === 0
        ) {
            return;
        }
        const userId = this.route.id;
        const summary = this.summaries.find((item) => item.user_id === userId);
        if (summary === undefined) {
            return;
        }
        const conversationId = summary.conversation_id;
        const limit = Math.min(HISTORY_PAGE_SIZE, this.messageOffset);
        const offset = this.messageOffset - limit;
        this.loadingOlderMessages = true;
        this.olderMessagesError = '';
        const messagesResult = await this.client.request<Message[]>('list_messages', {
            conversation_id: conversationId,
            limit,
            offset,
        });
        this.loadingOlderMessages = false;
        if (!messagesResult.ok) {
            this.olderMessagesError = messagesResult.error.message;
            this.notice = messagesResult.error.message;
            return;
        }
        if (this.route.kind !== 'chat' || this.route.id !== userId) {
            return;
        }
        const existing = new SvelteSet(this.messages.map((message) => message.id));
        const older = (messagesResult.value ?? [])
            .map((message) => this.normalizeMessage(message))
            .filter((message) => !existing.has(message.id));
        this.messages = [...older, ...this.messages];
        this.messageOffset = offset;
        this.hasOlderMessages = offset > 0;
    }

    private async markConversationRead(conversationId: string): Promise<void> {
        if (this.client === null) {
            return;
        }
        const result = await this.client.request('mark_conversation_read', {
            conversation_id: conversationId,
        });
        if (!result.ok) {
            this.notice = result.error.message;
            return;
        }
        this.setSummaryUnreadCount(conversationId, 0);
        this.scheduleRefresh();
    }

    private async markMessageRead(conversationId: string, messageIds: string[]): Promise<void> {
        if (this.client === null || messageIds.length === 0) {
            return;
        }
        const result = await this.client.request('mark_read', {
            conversation_id: conversationId,
            message_ids: messageIds,
        });
        if (!result.ok) {
            this.notice = result.error.message;
            return;
        }
        messageIds.forEach((messageId) => this.readMessageIds.add(messageId));
        this.decrementSummaryUnreadCount(conversationId, messageIds.length);
        this.scheduleRefresh();
    }

    private setSummaryUnreadCount(conversationId: string, unreadCount: number): void {
        this.summaries = this.summaries.map((summary) =>
            summary.conversation_id === conversationId
                ? { ...summary, unread_count: unreadCount }
                : summary,
        );
    }

    private decrementSummaryUnreadCount(conversationId: string, count: number): void {
        this.summaries = this.summaries.map((summary) =>
            summary.conversation_id === conversationId
                ? { ...summary, unread_count: Math.max(0, summary.unread_count - count) }
                : summary,
        );
    }

    async loadDetails(userId: string): Promise<void> {
        if (this.client === null) {
            return;
        }
        const detailsResult = await this.client.request<UserDetails>('get_user', {
            user_id: userId,
        });
        if (!detailsResult.ok) {
            this.notice = detailsResult.error.message;
            return;
        }
        const loaded = detailsResult.value;
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
        this.settingsLoad = 'loading';
        this.settingsLoadError = '';
        const valuesResult = await this.client.request<Record<string, string>>('list_settings');
        if (!valuesResult.ok) {
            this.settingsLoad = 'error';
            this.settingsLoadError = valuesResult.error.message;
            this.notice = valuesResult.error.message;
            return;
        }
        this.settings = { ...(valuesResult.value ?? {}) };
        this.settingsDraft = { ...this.settings };
        this.settingsLoad = 'ready';
        const autostartResult = await this.client.request<boolean>('autostart_get');
        this.autostart = autostartResult.ok ? autostartResult.value : false;
        this.autostartDraft = this.autostart;
        this.applyAppearance();
    }

    private async loadCall(): Promise<void> {
        if (this.client === null) {
            this.syncCallLevelPolling();
            return;
        }
        const callResult = await this.client.request<Call | null>('call_state');
        this.call = callResult.ok ? callResult.value : null;
        this.syncCallLevelPolling();
        void this.syncCallVolume();
    }

    private syncCallLevelPolling(): void {
        if (this.demo || this.client === null) {
            this.stopCallLevelPolling();
            this.callLevel = 0;
            return;
        }
        if (this.call === null || this.call.state !== 'in_call') {
            this.stopCallLevelPolling();
            this.callLevel = 0;
            return;
        }
        if (this.callLevelTimer !== null) {
            return;
        }
        this.callLevelTimer = window.setInterval(() => {
            void this.pollCallLevel();
        }, 50);
    }

    private stopCallLevelPolling(): void {
        if (this.callLevelTimer === null) {
            return;
        }
        window.clearInterval(this.callLevelTimer);
        this.callLevelTimer = null;
        this.callLevelPending = false;
    }

    private async pollCallLevel(): Promise<void> {
        if (this.client === null || this.callLevelPending) {
            return;
        }
        this.callLevelPending = true;
        const callId = this.call?.id ?? '';
        const metricsResult = await this.client.request<CallMetrics>('call_metrics');
        this.callLevelPending = false;
        if (this.call === null || this.call.state !== 'in_call' || this.call.id !== callId) {
            this.stopCallLevelPolling();
            this.callLevel = 0;
            return;
        }
        if (!metricsResult.ok) {
            this.callLevel = 0;
            return;
        }
        if (this.call.output_muted) {
            this.callLevel = 0;
            return;
        }
        const permille = metricsResult.value?.output_level ?? 0;
        this.callLevel = Math.min(1, Math.max(0, permille / 1000));
    }

    async loadCallLogs(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const callLogsResult = await this.client.request<CallLog[]>('list_call_logs', {
            user_id: '',
            limit: 100,
        });
        if (!callLogsResult.ok) {
            this.notice = callLogsResult.error.message;
            return;
        }
        this.callLogs = callLogsResult.value ?? [];
    }

    async loadAudioDevices(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const devicesResult = await this.client.request<AudioDevice[]>('list_audio_devices');
        if (!devicesResult.ok) {
            this.notice = devicesResult.error.message;
            return;
        }
        this.audioDevices = devicesResult.value ?? [];
    }

    private async loadRemote(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const remoteResult = await this.client.request<RemoteSession | null>('remote_session');
        this.remote = remoteResult.ok ? remoteResult.value : null;
    }

    private async loadParty(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const partyResult = await this.client.request<WatchParty | null>('watch_party');
        if (!partyResult.ok) {
            return;
        }
        const loaded = partyResult.value;
        this.adoptParty(loaded);
        if (loaded !== null) {
            this.prefetchYouTubeStream(loaded.video_id);
        }
    }

    private adoptParty(party: WatchParty | null): void {
        if (party === null) {
            this.party = null;
            return;
        }
        const current = this.party;
        if (current === null || current.party_id !== party.party_id) {
            this.party = party;
            return;
        }
        if (party.revision < current.revision) {
            return;
        }
        if (party.revision === current.revision && party.updated_at <= current.updated_at) {
            return;
        }
        this.party = party;
    }

    navigate(route: Route): void {
        if (route.kind !== 'remote') {
            this.remoteOutcome = null;
        }
        if (route.kind !== 'watch-party') {
            this.partyOutcome = null;
        }
        this.route = route;
        const hash = routeHash(route);
        if (window.location.hash !== hash) {
            window.location.hash = hash;
            return;
        }
        void this.loadRoute();
    }

    openWatchParty(userIds: string[] = []): void {
        const selfId = this.bootstrap?.identity.node_id ?? '';
        this.watchPartySelection = userIds.filter(
            (userId, index) => userId !== selfId && userIds.indexOf(userId) === index,
        );
        this.navigate({ kind: 'watch-party' });
    }

    takeWatchPartySelection(): string[] {
        const selection = this.watchPartySelection;
        this.watchPartySelection = [];
        return selection;
    }

    setSection(section: SettingsSection): void {
        this.navigate({ kind: 'settings', section });
    }

    backToChat(): void {
        const id =
            this.route.kind === 'watch-party'
                ? (this.party?.participants.find((item) => !this.isSelf(item)) ??
                  this.party?.host_id ??
                  this.lastChatUserId)
                : this.selectedUserId || this.lastChatUserId;
        this.navigate({ kind: 'chat', id });
    }

    applyAppearance(): void {
        if (typeof document === 'undefined') {
            return;
        }
        const appearance =
            this.settingsDraft['appearance.color_scheme'] ??
            this.settings['appearance.color_scheme'] ??
            this.bootstrap?.appearance ??
            'system';
        const root = document.documentElement;
        if (appearance === 'light' || appearance === 'dark') {
            root.dataset.theme = appearance;
            root.style.colorScheme = appearance;
        } else {
            delete root.dataset.theme;
            root.style.colorScheme = 'light dark';
        }

        this.watchSystemAppearance(appearance);
        this.syncThemeColor(appearance);
    }

    private syncThemeColor(appearance: string): void {
        if (typeof document === 'undefined') {
            return;
        }

        const resolved =
            appearance === 'system'
                ? typeof window !== 'undefined' &&
                  typeof window.matchMedia === 'function' &&
                  window.matchMedia('(prefers-color-scheme: light)').matches
                    ? 'light'
                    : 'dark'
                : appearance;
        document
            .querySelector('meta[name="theme-color"]')
            ?.setAttribute('content', resolved === 'light' ? '#f4efe2' : '#182b2f');
    }

    private watchSystemAppearance(appearance: string): void {
        this.removeAppearanceMediaListener();
        if (
            appearance !== 'system' ||
            typeof window === 'undefined' ||
            typeof window.matchMedia !== 'function'
        ) {
            return;
        }

        const media = window.matchMedia('(prefers-color-scheme: light)');
        const listener = () => this.syncThemeColor('system');
        media.addEventListener('change', listener);
        this.appearanceMedia = media;
        this.appearanceMediaListener = listener;
    }

    private removeAppearanceMediaListener(): void {
        if (this.appearanceMedia !== null && this.appearanceMediaListener !== null) {
            this.appearanceMedia.removeEventListener('change', this.appearanceMediaListener);
        }
        this.appearanceMedia = null;
        this.appearanceMediaListener = null;
    }

    async loadStorageUsage(): Promise<void> {
        if (this.client === null) {
            return;
        }
        const usageResult = await this.client.request<StorageUsage>('storage_usage');
        if (!usageResult.ok) {
            this.notice = usageResult.error.message;
            return;
        }
        if (usageResult.value !== null) {
            this.storageUsage = usageResult.value;
        }
    }

    async clearHistory(userId = ''): Promise<void> {
        if (this.client === null) {
            this.notice = 'History clearing is not available in the demo.';
            return;
        }
        const clearResult = await this.client.request('clear_history', { user_id: userId });
        if (!clearResult.ok) {
            this.notice = clearResult.error.message;
            return;
        }
        this.notice = userId === '' ? 'Local history cleared.' : 'Conversation history cleared.';
        this.releaseAttachments();
        this.messages = [];
        this.messagesLoad = 'empty';
        this.messagesError = '';
        this.olderMessagesError = '';
        this.messageOffset = 0;
        this.hasOlderMessages = false;
        await this.refresh();
        await this.loadStorageUsage();
        await this.loadCallLogs();
        if (this.route.kind === 'settings') {
            return;
        }
        if (this.route.kind === 'chat' && this.route.id !== '') {
            await this.loadMessages(this.route.id);
        } else if (this.route.kind === 'profile' && this.route.id !== '') {
            await this.loadDetails(this.route.id);
        }
    }

    async startRecording(): Promise<void> {
        if (this.recording) {
            return;
        }
        if (typeof navigator === 'undefined' || navigator.mediaDevices === undefined) {
            this.notice = 'Microphone capture is not available.';
            return;
        }
        if (typeof MediaRecorder === 'undefined') {
            this.notice = 'Voice recording is not supported by this webview.';
            return;
        }
        const streamResult = await attemptAsync<MediaStream, Error>(() =>
            navigator.mediaDevices.getUserMedia({ audio: true }),
        );
        if (!streamResult.ok) {
            this.releaseRecording();
            this.notice = `Microphone unavailable: ${streamResult.error.message}`;
            return;
        }
        const stream = streamResult.value;
        this.recordingStream = stream;
        const recordingInput = await this.prepareRecordingInput(stream);
        const candidates = ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus'];
        const mimeType = candidates.find((candidate) => MediaRecorder.isTypeSupported(candidate));
        this.recorder =
            mimeType === undefined
                ? new MediaRecorder(recordingInput)
                : new MediaRecorder(recordingInput, { mimeType });
        this.recordingChunks = [];
        this.discardRecording = false;
        this.recorder.ondataavailable = (event) => {
            if (event.data.size > 0) {
                this.recordingChunks.push(event.data);
            }
        };
        this.recorder.onstop = () => {
            const recorder = this.recorder;
            const type = recorder?.mimeType || 'audio/webm';
            const chunks = this.recordingChunks;
            this.releaseRecording();
            if (this.discardRecording || chunks.length === 0) {
                return;
            }
            const extension = type.includes('ogg') ? 'ogg' : 'webm';
            const file = new File(
                [new Blob(chunks, { type })],
                `voice-${Date.now()}.${extension}`,
                { type },
            );
            void this.sendFile(file);
        };
        this.recorder.start();
        this.recording = true;
        this.recordingSeconds = 0;
        this.recordingTimer = window.setInterval(() => {
            this.recordingSeconds += 1;
            if (this.recordingSeconds >= 300) {
                void this.stopRecording();
            }
        }, 1_000);
        void this.startRecordingLevel();
    }

    async stopRecording(): Promise<void> {
        if (!this.recording || this.recorder === null) {
            return;
        }
        if (this.recorder.state !== 'inactive') {
            this.recorder.stop();
        } else {
            this.releaseRecording();
        }
    }

    cancelRecording(): void {
        this.discardRecording = true;
        if (this.recorder !== null && this.recorder.state !== 'inactive') {
            this.recorder.stop();
        } else {
            this.releaseRecording();
        }
    }

    private async prepareRecordingInput(stream: MediaStream): Promise<MediaStream> {
        if (typeof AudioContext === 'undefined') {
            return stream;
        }
        const contextResult = attempt(() => new AudioContext());
        if (!contextResult.ok) {
            return stream;
        }
        const context = contextResult.value;
        const resumeResult = await attemptAsync(() => context.resume());
        if (!resumeResult.ok || context.state === 'suspended') {
            void context.close();
            this.notice = 'Microphone level is unavailable.';
            return stream;
        }
        const sourceResult = attempt(() => context.createMediaStreamSource(stream));
        const gainResult = attempt(() => context.createGain());
        const destinationResult = attempt(() => context.createMediaStreamDestination());
        if (!sourceResult.ok || !gainResult.ok || !destinationResult.ok) {
            void context.close();
            return stream;
        }
        gainResult.value.gain.value = this.audioInputVolume() / MAX_VOLUME;
        sourceResult.value.connect(gainResult.value);
        gainResult.value.connect(destinationResult.value);
        this.recordingContext = context;
        this.recordingSource = sourceResult.value;
        return destinationResult.value.stream;
    }

    private async startRecordingLevel(): Promise<void> {
        const context = this.recordingContext;
        const source = this.recordingSource;
        if (context === null || source === null) {
            return;
        }
        const analyser = context.createAnalyser();
        analyser.fftSize = 512;
        source.connect(analyser);
        const samples = new Uint8Array(analyser.fftSize);
        this.recordingLevelTimer = window.setInterval(() => {
            analyser.getByteTimeDomainData(samples);
            const energy = samples.reduce((total, sample) => {
                const deviation = (sample - 128) / 128;
                return total + deviation * deviation;
            }, 0);
            const target = Math.min(1, Math.sqrt(energy / samples.length) * RECORDING_RMS_GAIN);
            const factor =
                target > this.recordingLevel ? RECORDING_LEVEL_ATTACK : RECORDING_LEVEL_RELEASE;
            this.recordingLevel = this.recordingLevel + (target - this.recordingLevel) * factor;
            this.recordingWaveform = pushWaveSample(this.recordingWaveform, this.recordingLevel);
        }, RECORDING_LEVEL_SAMPLE_MS);
    }

    private releaseRecording(): void {
        if (this.recordingTimer !== null) {
            window.clearInterval(this.recordingTimer);
            this.recordingTimer = null;
        }
        if (this.recordingLevelTimer !== null) {
            window.clearInterval(this.recordingLevelTimer);
            this.recordingLevelTimer = null;
        }
        this.recordingSource = null;
        if (this.recordingContext !== null) {
            void this.recordingContext.close();
            this.recordingContext = null;
        }
        this.recordingStream?.getTracks().forEach((track) => track.stop());
        this.recordingStream = null;
        this.recorder = null;
        this.recording = false;
        this.recordingSeconds = 0;
        this.recordingLevel = 0;
        this.recordingWaveform = [];
    }

    async retryMessage(messageId: string): Promise<void> {
        if (this.messageRetryPending !== '') {
            return;
        }
        if (this.demo) {
            this.notice = 'Message retry is not available in the demo.';
            return;
        }
        if (this.client === null) {
            return;
        }
        this.messageRetryPending = messageId;
        const result = await this.client.request<boolean>('retry_message', {
            message_id: messageId,
        });
        this.messageRetryPending = '';
        if (!result.ok) {
            this.notice = result.error.message;
            return;
        }
        if (result.value !== true) {
            this.notice = 'This message cannot be retried.';
            return;
        }
        this.applyMessageState(messageId, 'queued');
        this.notice = 'Message queued for retry.';
    }

    async editMessage(messageId: string, body: string): Promise<boolean> {
        if (this.messageActionPending !== '' || body.trim() === '') {
            return false;
        }
        if (this.demo) {
            const index = this.messages.findIndex((message) => message.id === messageId);
            if (index < 0) {
                return false;
            }
            const message = this.messages[index];
            if (message === undefined || message.deleted_at !== 0) {
                return false;
            }
            const next = this.messages.slice();
            next[index] = { ...message, body, edited_at: Date.now() };
            this.messages = next;
            this.scheduleRefresh();
            return true;
        }
        if (this.client === null) {
            this.notice = 'Message editing is not available in the demo.';
            return false;
        }
        this.messageActionPending = messageId;
        const result = await this.client.request('edit_message', {
            message_id: messageId,
            body,
        });
        this.messageActionPending = '';
        if (!result.ok) {
            this.notice = result.error.message;
            return false;
        }
        return true;
    }

    async deleteMessage(messageId: string): Promise<boolean> {
        if (this.messageActionPending !== '') {
            return false;
        }
        if (this.demo) {
            const index = this.messages.findIndex((message) => message.id === messageId);
            if (index < 0) {
                return false;
            }
            const message = this.messages[index];
            if (message === undefined || message.deleted_at !== 0) {
                return false;
            }
            const next = this.messages.slice();
            next[index] = { ...message, deleted_at: Date.now() };
            this.messages = next;
            this.scheduleRefresh();
            return true;
        }
        if (this.client === null) {
            this.notice = 'Message deletion is not available in the demo.';
            return false;
        }
        this.messageActionPending = messageId;
        const result = await this.client.request('delete_message', {
            message_id: messageId,
        });
        this.messageActionPending = '';
        if (!result.ok) {
            this.notice = result.error.message;
            return false;
        }
        return true;
    }

    private sendBlockedReason(userId: string): string {
        if (this.connection !== 'online') {
            return 'Messages cannot be sent while the daemon is offline.';
        }
        const user = this.users.find((item) => item.id === userId);
        if (user?.policy === 'ignore') {
            return 'This user is blocked. Unblock them from Profile to send messages.';
        }
        return '';
    }

    async sendMessage(): Promise<void> {
        if (this.route.kind !== 'chat' || this.draft.trim() === '') {
            return;
        }
        const body = this.draft;
        const userId = this.route.id;
        const replyTo = this.replyToId;
        const blocked = this.sendBlockedReason(userId);
        if (blocked !== '') {
            this.notice = blocked;
            return;
        }
        if (messageBodyBytes(body) > MAX_MESSAGE_BODY_BYTES) {
            this.notice = 'Message is too long. Shorten it before sending.';
            return;
        }
        this.draft = '';
        await this.setTyping(false);
        if (this.demo) {
            const message: Message = {
                id: `demo-${Date.now()}`,
                conversation_id: 'conv-mika',
                user_id: userId,
                sender_id: this.bootstrap?.identity.node_id ?? '',
                local_sender: true,
                seq: this.messages.length + 1,
                sent_at: Date.now(),
                received_at: Date.now(),
                body,
                reply_to: replyTo,
                attachment_hash: '',
                attachment_name: '',
                attachment_size: 0,
                attachment_mime: '',
                attachment_path: '',
                state: 'sent',
                edited_at: 0,
                deleted_at: 0,
                notify: false,
            };
            this.replyToId = '';
            this.appendMessage(message);
            this.updateSummary(message);
            return;
        }
        if (this.client === null) {
            return;
        }
        const messageResult = await this.client.request<string>('send_message', {
            user_id: userId,
            body,
            reply_to: replyTo,
        });
        if (!messageResult.ok) {
            this.notice = messageResult.error.message;
            return;
        }
        this.replyToId = '';
        const messageId = messageResult.value;
        if (messageId === null) {
            this.notice = 'send message returned no message id';
            return;
        }
        await this.loadSentMessage(messageId, userId);
    }

    async sendFile(file: File): Promise<void> {
        if (this.route.kind !== 'chat') {
            return;
        }
        const userId = this.route.id;
        const replyTo = this.replyToId;
        const blocked = this.sendBlockedReason(userId);
        if (blocked !== '') {
            this.notice = blocked;
            return;
        }
        if (this.demo) {
            const message: Message = {
                id: `demo-${Date.now()}`,
                conversation_id: 'conv-mika',
                user_id: userId,
                sender_id: this.bootstrap?.identity.node_id ?? '',
                local_sender: true,
                seq: this.messages.length + 1,
                sent_at: Date.now(),
                received_at: Date.now(),
                body: '',
                reply_to: replyTo,
                attachment_hash: 'demo',
                attachment_name: file.name,
                attachment_size: file.size,
                attachment_mime: file.type,
                attachment_path: '',
                state: 'sent',
                edited_at: 0,
                deleted_at: 0,
                notify: false,
            };
            this.replyToId = '';
            this.appendMessage(message);
            this.updateSummary(message);
            this.notice = 'Attachment preview added for this demo.';
            return;
        }
        if (this.client === null) {
            return;
        }
        const attachmentResult = await this.client.uploadAttachment(userId, file, replyTo);
        if (!attachmentResult.ok) {
            this.notice = attachmentResult.error.message;
            return;
        }
        const messageId = attachmentResult.value;
        if (messageId === null) {
            this.notice = 'attachment upload returned no message id';
            return;
        }
        await this.loadSentMessage(messageId, userId);
    }

    async uploadAvatar(file: File): Promise<void> {
        this.selfAvatarUrl = URL.createObjectURL(file);
        if (this.client === null || this.demo) {
            this.notice = 'Avatar preview updated for this demo.';
            return;
        }
        const uploadResult = await this.client.uploadAvatar(file);
        if (!uploadResult.ok) {
            this.notice = uploadResult.error.message;
            return;
        }
        this.notice = 'Avatar updated.';
        const bootstrapResult = await this.client.request<Bootstrap>('bootstrap');
        if (bootstrapResult.ok) {
            const bootstrap = bootstrapResult.value;
            if (bootstrap !== null) {
                this.bootstrap = bootstrap;
                this.syncAvatars();
            }
        }
    }

    async saveAttachment(hash: string, name: string): Promise<void> {
        if (this.client === null || hash === '') {
            this.notice = 'This attachment is not available in the demo.';
            return;
        }
        const downloadResult = await this.client.downloadAttachment(hash);
        if (!downloadResult.ok) {
            this.notice = downloadResult.error.message;
            return;
        }
        const url = downloadResult.value;
        if (url === null) {
            return;
        }
        const anchor = document.createElement('a');
        anchor.href = url;
        anchor.download = name || 'attachment';
        anchor.rel = 'noopener';
        document.body.appendChild(anchor);
        anchor.click();
        anchor.remove();
        window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
    }

    async openAttachment(hash: string): Promise<void> {
        await this.saveAttachment(hash, 'attachment');
    }

    async attachmentUrl(hash: string): Promise<string> {
        if (hash === '') {
            return '';
        }
        const cached = this.attachmentUrls.get(hash);
        if (cached !== undefined) {
            return cached;
        }
        const pending = this.attachmentPending.get(hash);
        if (pending !== undefined) {
            return pending;
        }
        if (this.client === null) {
            return '';
        }
        const task = this.fetchAttachmentUrl(hash).finally(() => {
            this.attachmentPending.delete(hash);
        });
        this.attachmentPending.set(hash, task);
        return task;
    }

    private async fetchAttachmentUrl(hash: string): Promise<string> {
        if (this.client === null) {
            return '';
        }
        const downloadResult = await this.client.downloadAttachment(hash);
        if (!downloadResult.ok) {
            this.notice = downloadResult.error.message;
            return '';
        }
        const url = downloadResult.value;
        if (url === null) {
            this.notice = 'Attachment download failed.';
            return '';
        }
        this.attachmentUrls.set(hash, url);
        return url;
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

    async setUserVolume(userId: string, volume: number): Promise<void> {
        const next = clampVolume(volume);
        this.preferences = {
            ...this.preferences,
            userVolumes: { ...this.preferences.userVolumes, [userId]: next },
        };
        savePreferences(this.preferences);
        if (this.client === null || this.call?.user_id !== userId) {
            return;
        }
        await this.syncCallVolume();
    }

    async updatePolicy(userId: string, policy: UserPolicy): Promise<void> {
        if (this.client === null) {
            this.users = this.users.map((user) =>
                user.id === userId ? { ...user, policy } : user,
            );
            return;
        }
        const policyResult = await this.client.request('set_user_policy', {
            user_id: userId,
            policy,
        });
        if (!policyResult.ok) {
            this.notice = policyResult.error.message;
            return;
        }
        await this.refresh();
        if (this.details?.user.id === userId) {
            await this.loadDetails(userId);
        }
    }

    async blockUser(userId: string): Promise<void> {
        if (this.client !== null) {
            const blockResult = await this.client.request('block_user', { user_id: userId });
            if (!blockResult.ok) {
                this.notice = blockResult.error.message;
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
                input_muted: false,
                output_muted: false,
            };
            this.navigate({ kind: 'call', id: userId });
            this.syncCallLevelPolling();
            return;
        }
        const callResult = await this.client.request<string>('start_call', { user_id: userId });
        if (!callResult.ok) {
            this.notice = callResult.error.message;
            this.call = null;
            return;
        }
        await this.loadCall();
        this.navigate({ kind: 'call', id: userId });
    }

    async acceptCall(): Promise<void> {
        if (this.client === null || this.call === null || this.callActionPending) {
            return;
        }
        const callId = this.call.id;
        this.callActionPending = true;
        const acceptResult = await this.client.request('accept_call', { call_id: callId });
        this.callActionPending = false;
        if (!acceptResult.ok) {
            await this.loadCall();
            if (this.call?.id !== callId) {
                this.notice = 'This call is no longer available.';
                return;
            }
            this.notice = acceptResult.error.message;
            return;
        }
        await this.loadCall();
        const activeCall = this.call;
        if (activeCall === null || activeCall.id !== callId) {
            this.notice = 'This call is no longer available.';
            return;
        }
        this.navigate({ kind: 'call', id: activeCall.user_id });
    }

    async declineCall(): Promise<void> {
        if (this.client === null || this.call === null) {
            this.call = null;
            this.syncCallLevelPolling();
            return;
        }
        if (this.callActionPending) {
            return;
        }
        const callId = this.call.id;
        this.callActionPending = true;
        const declineResult = await this.client.request('decline_call', { call_id: callId });
        this.callActionPending = false;
        if (!declineResult.ok) {
            await this.loadCall();
            if (this.call !== null) {
                this.notice = declineResult.error.message;
            }
            return;
        }
        if (this.call?.id === callId) {
            this.call = null;
            this.syncCallLevelPolling();
        }
    }

    async hangupCall(): Promise<void> {
        if (this.client === null || this.call === null) {
            this.call = null;
            this.syncCallLevelPolling();
            return;
        }
        if (this.callActionPending) {
            return;
        }
        const callId = this.call.id;
        this.callActionPending = true;
        const hangupResult = await this.client.request('hangup_call', { call_id: callId });
        this.callActionPending = false;
        if (!hangupResult.ok) {
            await this.loadCall();
            if (this.call !== null) {
                this.notice = hangupResult.error.message;
            }
            return;
        }
        if (this.call?.id === callId) {
            this.call = null;
            this.syncCallLevelPolling();
        }
    }

    async toggleInputMute(): Promise<void> {
        if (this.call === null) {
            return;
        }
        await this.setCallMuted(!this.call.input_muted, this.call.output_muted);
    }

    async toggleOutputMute(): Promise<void> {
        if (this.call === null) {
            return;
        }
        await this.setCallMuted(this.call.input_muted, !this.call.output_muted);
    }

    private async setCallMuted(inputMuted: boolean, outputMuted: boolean): Promise<void> {
        if (this.client === null || this.call === null) {
            this.call =
                this.call === null
                    ? null
                    : { ...this.call, input_muted: inputMuted, output_muted: outputMuted };
            return;
        }
        const muteResult = await this.client.request('set_call_muted', {
            call_id: this.call.id,
            input_muted: inputMuted,
            output_muted: outputMuted,
        });
        if (!muteResult.ok) {
            this.notice = muteResult.error.message;
            return;
        }
        if (this.call !== null) {
            this.call = { ...this.call, input_muted: inputMuted, output_muted: outputMuted };
        }
    }

    async requestRemote(userId: string): Promise<void> {
        this.remoteOutcome = null;
        if (this.client === null) {
            this.remote = {
                session_id: 'demo-remote',
                user_id: userId,
                fingerprint: '',
                role: 'viewer',
                state: 'requested',
                indicator: false,
                input_allowed: true,
                expires_at: nowSeconds() + 300,
            };
            this.navigate({ kind: 'remote', id: userId });
            return;
        }
        const remoteResult = await this.client.request<RemoteSession>('request_remote_session', {
            user_id: userId,
        });
        if (!remoteResult.ok) {
            this.notice = remoteResult.error.message;
            return;
        }
        await this.loadRemote();
        this.navigate({ kind: 'remote', id: userId });
    }

    async acceptRemote(): Promise<void> {
        this.remoteOutcome = null;
        if (this.client === null || this.remote === null) {
            if (this.remote !== null) {
                this.remote = { ...this.remote, state: 'active', indicator: true };
            }
            return;
        }
        const acceptResult = await this.client.request('accept_remote_session', {
            session_id: this.remote.session_id,
        });
        if (!acceptResult.ok) {
            this.notice = acceptResult.error.message;
            return;
        }
        await this.loadRemote();
    }

    async denyRemote(): Promise<void> {
        if (this.client === null || this.remote === null) {
            this.finishRemote('denied');
            return;
        }
        await this.client.request('deny_remote_session', {
            session_id: this.remote.session_id,
        });
        this.finishRemote('denied');
    }

    async setRemoteInput(allowed: boolean): Promise<void> {
        if (this.remote === null || this.remoteInputPending) {
            return;
        }
        if (this.client === null) {
            this.remote = { ...this.remote, input_allowed: allowed };
            return;
        }
        this.remoteInputPending = true;
        const result = await this.client.request('set_remote_input', {
            session_id: this.remote.session_id,
            allowed,
        });
        this.remoteInputPending = false;
        if (!result.ok) {
            this.notice = result.error.message;
            return;
        }
        if (this.remote !== null) {
            this.remote = { ...this.remote, input_allowed: allowed };
        }
    }

    async endRemote(): Promise<void> {
        if (this.client === null || this.remote === null) {
            this.finishRemote('left');
            return;
        }
        const role = this.remote.role;
        await this.client.request('end_remote_session', {
            session_id: this.remote.session_id,
        });
        this.finishRemote(role === 'host' ? 'ended' : 'left');
    }

    async startParty(videoId: string, userIds: string[]): Promise<void> {
        this.partyOutcome = null;
        const selfId = this.bootstrap?.identity.node_id ?? '';
        if (this.client === null) {
            this.party = {
                party_id: 'demo-party',
                host_id: selfId,
                video_id: videoId,
                role: 'host',
                state: 'active',
                participants: [selfId, ...userIds],
                invited: [],
                position_ms: 0,
                playing: true,
                revision: 1,
                actor: selfId,
                updated_at: nowSeconds(),
            };
            this.openPlayer(videoId, 0, true);
            this.navigate({ kind: 'watch-party' });
            return;
        }
        const partyResult = await this.client.request<WatchParty>('start_watch_party', {
            user_ids: userIds,
            video_id: videoId,
        });
        if (!partyResult.ok) {
            this.notice = partyResult.error.message;
            return;
        }
        this.adoptParty(partyResult.value);
        this.prefetchYouTubeStream(videoId);
        this.navigate({ kind: 'watch-party' });
    }

    async inviteParty(userIds: string[]): Promise<void> {
        if (this.client === null || this.party === null) {
            return;
        }
        const partyResult = await this.client.request<WatchParty>('invite_watch_party', {
            party_id: this.party.party_id,
            user_ids: userIds,
        });
        if (!partyResult.ok) {
            this.notice = partyResult.error.message;
            return;
        }
        this.adoptParty(partyResult.value);
    }

    async youtubePreview(videoId: string): Promise<YoutubePreview | null> {
        const cached = this.videoPreviews.get(videoId);
        if (cached !== undefined) {
            return cached;
        }
        if (this.client === null) {
            const preview: YoutubePreview = {
                video_id: videoId,
                title: `YouTube video ${videoId}`,
                channel: 'YouTube',
                description: '',
                thumbnail_url: `https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`,
                thumbnail_data: null,
                duration_seconds: 0,
            };
            this.videoPreviews.set(videoId, preview);
            return preview;
        }
        const previewResult = await this.client.request<YoutubePreview>('youtube_preview', {
            video_id: videoId,
        });
        if (!previewResult.ok) {
            return null;
        }
        const preview = previewResult.value;
        if (preview === null) {
            return null;
        }
        this.videoPreviews.set(videoId, preview);
        return preview;
    }

    async youtubeStream(videoId: string): Promise<string | null> {
        return this.loadYoutubeStream(videoId, true);
    }

    prefetchYouTubeStream(videoId: string): void {
        if (
            videoId === '' ||
            this.client === null ||
            this.preparedVideos.has(videoId) ||
            this.prefetchAttempts.has(videoId) ||
            this.videoStreamPending.has(videoId)
        ) {
            return;
        }
        this.prefetchAttempts.add(videoId);
        void this.loadYoutubeStream(videoId, false);
    }

    videoPreparing(videoId: string): boolean {
        return this.preparingVideos.has(videoId);
    }

    private loadYoutubeStream(videoId: string, notify: boolean): Promise<string | null> {
        const pending = this.videoStreamPending.get(videoId);
        if (pending !== undefined) {
            return pending;
        }
        const task = this.fetchYoutubeStream(videoId, notify).finally(() => {
            this.videoStreamPending.delete(videoId);
            this.preparingVideos.delete(videoId);
        });
        this.preparingVideos.add(videoId);
        this.videoStreamPending.set(videoId, task);
        return task;
    }

    private async fetchYoutubeStream(videoId: string, notify: boolean): Promise<string | null> {
        if (this.client === null) {
            return null;
        }
        const streamResult = await this.client.request<YoutubeStream>('youtube_stream', {
            video_id: videoId,
        });
        if (!streamResult.ok) {
            if (notify) {
                this.notice = `Video stream unavailable: ${streamResult.error.message}`;
            }
            return null;
        }
        const stream = streamResult.value;
        if (stream === null) {
            if (notify) {
                this.notice = 'Video stream unavailable.';
            }
            return null;
        }
        this.preparedVideos.add(videoId);
        return stream.url;
    }

    async sendVideoToUsers(userIds: string[], videoId: string): Promise<void> {
        if (userIds.length === 0) {
            this.notice = 'Select at least one person.';
            return;
        }
        const body = `https://www.youtube.com/watch?v=${videoId}`;
        if (this.client === null) {
            this.notice = 'Video link sent for this demo.';
            return;
        }
        let failed = 0;
        for (const userId of userIds) {
            const sendResult = await this.client.request<string>('send_message', {
                user_id: userId,
                body,
            });
            if (!sendResult.ok) {
                failed += 1;
            }
        }
        this.notice =
            failed === 0
                ? `Video sent to ${userIds.length} ${userIds.length === 1 ? 'person' : 'people'}.`
                : `Video sent to ${userIds.length - failed} of ${userIds.length} people.`;
    }

    async acceptParty(): Promise<void> {
        this.partyOutcome = null;
        if (this.client === null || this.party === null) {
            if (this.party !== null) {
                this.party = { ...this.party, state: 'active' };
            }
            return;
        }
        const partyResult = await this.client.request<WatchParty>('accept_watch_party', {
            party_id: this.party.party_id,
        });
        if (!partyResult.ok) {
            this.notice = partyResult.error.message;
            return;
        }
        const party = partyResult.value;
        if (party !== null) {
            this.adoptParty(party);
            this.prefetchYouTubeStream(party.video_id);
        }
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
        const partyResult = await this.client.request<WatchParty>('publish_watch_party', {
            party_id: this.party.party_id,
            position_ms: positionMs,
            playing,
        });
        if (partyResult.ok && partyResult.value !== null) {
            this.adoptParty(partyResult.value);
        }
    }

    async leaveParty(): Promise<void> {
        if (this.client !== null && this.party !== null) {
            await this.client.request('leave_watch_party', { party_id: this.party.party_id });
        }
        this.party = null;
        this.closePlayer();
        this.backToChat();
    }

    async endParty(): Promise<void> {
        if (this.party !== null && this.party.role === 'guest') {
            await this.leaveParty();
            return;
        }
        if (this.client !== null && this.party !== null) {
            await this.client.request('end_watch_party', { party_id: this.party.party_id });
        }
        this.party = null;
        this.closePlayer();
        this.backToChat();
    }

    setDraftSetting(key: string, value: string): void {
        this.settingsDraft = { ...this.settingsDraft, [key]: value };
        this.settingsSave = 'dirty';
        this.settingsSaveError = '';
    }

    setDraftSettingLive(key: string, value: string): void {
        this.setDraftSetting(key, value);
        if (this.client === null) {
            this.settings = { ...this.settings, [key]: value };
            this.settingsSave = this.settingsDirty ? 'dirty' : 'saved';
            return;
        }
        this.pendingLiveSettings.set(key, value);
        if (this.liveSettingTimers.has(key)) {
            return;
        }
        const timer = window.setTimeout(() => {
            this.liveSettingTimers.delete(key);
            const pending = this.pendingLiveSettings.get(key);
            this.pendingLiveSettings.delete(key);
            if (pending !== undefined) {
                void this.saveLiveSetting(key, pending);
            }
        }, LIVE_SETTING_SAVE_DELAY_MS);
        this.liveSettingTimers.set(key, timer);
    }

    private async saveLiveSetting(key: string, value: string): Promise<void> {
        const client = this.client;
        if (client === null) {
            return;
        }
        const result = await client.request('settings_save', { key, value });
        if (!result.ok) {
            this.settingsSave = 'error';
            this.settingsSaveError = result.error.message;
            return;
        }
        this.settings = { ...this.settings, [key]: value };
        if (!this.settingsDirty) {
            this.settingsSave = 'saved';
            this.settingsSaveError = '';
        }
    }

    private cancelLiveSettings(): void {
        for (const timer of this.liveSettingTimers.values()) {
            window.clearTimeout(timer);
        }
        this.liveSettingTimers.clear();
        this.pendingLiveSettings.clear();
    }

    setAutostart(value: boolean): void {
        this.autostartDraft = value;
        this.settingsSave = 'dirty';
        this.settingsSaveError = '';
    }

    markSettingsDirty(): void {
        this.settingsSave = 'dirty';
        this.settingsSaveError = '';
    }

    updatePreferences(patch: Partial<Preferences>): void {
        this.preferences = { ...this.preferences, ...patch };
        savePreferences(this.preferences);
        this.settingsSave = 'dirty';
        this.settingsSaveError = '';
    }

    async saveSettings(): Promise<void> {
        this.cancelLiveSettings();
        if (this.client === null) {
            this.settings = { ...this.settingsDraft };
            this.autostart = this.autostartDraft;
            this.settingsSave = 'saved';
            this.settingsSaveError = '';
            this.notice = 'Settings saved for this demo.';
            return;
        }
        this.settingsSave = 'saving';
        this.settingsSaveError = '';
        for (const [key, value] of Object.entries(this.settingsDraft)) {
            if (this.settings[key] === value) {
                continue;
            }
            const settingsResult = await this.client.request('settings_save', { key, value });
            if (!settingsResult.ok) {
                this.settingsSave = 'error';
                this.settingsSaveError = settingsResult.error.message;
                this.notice = settingsResult.error.message;
                return;
            }
        }
        if (this.autostart !== this.autostartDraft) {
            const autostartResult = await this.client.request('autostart_set', {
                enabled: this.autostartDraft,
            });
            if (!autostartResult.ok) {
                this.settingsSave = 'error';
                this.settingsSaveError = autostartResult.error.message;
                this.notice = autostartResult.error.message;
                return;
            }
        }
        this.settingsSave = 'saved';
        this.settingsSaveError = '';
        this.notice = 'Settings saved.';
        await this.loadSettings();
        this.applyAppearance();
        await this.loadStorageUsage();
        this.bootstrap =
            this.bootstrap === null ? null : { ...this.bootstrap, autostart: this.autostart };
    }

    revertSettings(): void {
        this.cancelLiveSettings();
        this.settingsDraft = { ...this.settings };
        this.autostartDraft = this.autostart;
        this.settingsSave = 'idle';
        this.settingsSaveError = '';
        this.applyAppearance();
    }

    openPlayer(videoId: string, positionMs = 0, playing = true): void {
        this.player = {
            videoId,
            title: `YouTube video ${videoId}`,
            channel: 'YouTube',
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
        this.clearToastTimer(id);
        this.toasts = this.toasts.filter((toast) => toast.id !== id);
    }

    private toastTimeoutMs(): number {
        const configured = Number.parseInt(this.settings['overlay.timeout_secs'] ?? '', 10);
        const seconds = Number.isFinite(configured) ? configured : DEFAULT_TOAST_TIMEOUT_SECS;
        return Math.min(60, Math.max(1, seconds)) * 1_000;
    }

    private clearToastTimer(id: string): void {
        const timer = this.toastTimers.get(id);
        if (timer !== undefined) {
            window.clearTimeout(timer);
            this.toastTimers.delete(id);
        }
    }

    private pushToast(toast: Toast): void {
        this.clearToastTimer(toast.id);
        const toasts = [...this.toasts.filter((item) => item.id !== toast.id), toast].slice(
            -TOAST_LIMIT,
        );
        const kept = new SvelteSet(toasts.map((item) => item.id));
        for (const id of [...this.toastTimers.keys()]) {
            if (!kept.has(id)) {
                this.clearToastTimer(id);
            }
        }
        this.toasts = toasts;
        this.toastTimers.set(
            toast.id,
            window.setTimeout(() => this.dismissToast(toast.id), this.toastTimeoutMs()),
        );
    }

    dismissNotice(): void {
        this.notice = '';
    }

    transferFor(hash: string): TransferState | null {
        return this.transfers[hash] ?? null;
    }

    private setTransfer(hash: string, bytes: number, total: number): void {
        if (hash === '') {
            return;
        }
        this.transfers = {
            ...this.transfers,
            [hash]: { bytes, total, done: false, failed: '' },
        };
    }

    private finishTransfer(hash: string, reason: string): void {
        if (hash === '') {
            return;
        }
        const current = this.transfers[hash] ?? { bytes: 0, total: 0, done: false, failed: '' };
        this.transfers = {
            ...this.transfers,
            [hash]: { ...current, done: true, failed: reason },
        };
        if (reason !== '') {
            this.notice = `Attachment transfer failed: ${reason}`;
        }
    }

    private handleEvent(event: WebSocketEvent): void {
        switch (event.type) {
            case 'connected':
                this.connection = 'online';
                this.errorMessage = '';
                void this.refresh();
                this.syncCallLevelPolling();
                break;
            case 'disconnected':
                this.connection = 'offline';
                this.stopCallLevelPolling();
                this.callLevel = 0;
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
                this.applyMessageState(String(event.message_id ?? ''), String(event.state ?? ''));
                break;
            }
            case 'message_edited': {
                const messageId = String(event.message_id ?? '');
                const index = this.messages.findIndex((message) => message.id === messageId);
                const message = this.messages[index];
                if (message !== undefined) {
                    const next = this.messages.slice();
                    next[index] = {
                        ...message,
                        body: String(event.body ?? ''),
                        edited_at: Number(event.edited_at ?? 0),
                    };
                    this.messages = next;
                }
                this.scheduleRefresh();
                break;
            }
            case 'message_deleted': {
                const messageId = String(event.message_id ?? '');
                const index = this.messages.findIndex((message) => message.id === messageId);
                const message = this.messages[index];
                if (message !== undefined) {
                    const next = this.messages.slice();
                    next[index] = {
                        ...message,
                        deleted_at: Math.floor(Date.now() / 1_000),
                    };
                    this.messages = next;
                }
                this.scheduleRefresh();
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
            case 'candidates':
            case 'candidate_lost':
                void this.refreshNetwork();
                break;
            case 'conversation_changed':
            case 'user_connected':
            case 'user_disconnected':
            case 'users':
            case 'policy_changed':
                this.scheduleRefresh();
                break;
            case 'incoming_call':
                this.call = event.call as Call;
                this.syncCallLevelPolling();
                break;
            case 'call_state_changed':
                if (this.call !== null && this.call.id === String(event.call_id ?? '')) {
                    this.call = { ...this.call, state: String(event.state ?? this.call.state) };
                }
                this.syncCallLevelPolling();
                void this.syncCallVolume();
                break;
            case 'call_mute_changed':
                if (this.call !== null && this.call.id === String(event.call_id ?? '')) {
                    this.call = {
                        ...this.call,
                        input_muted: Boolean(event.input_muted),
                        output_muted: Boolean(event.output_muted),
                    };
                }
                break;
            case 'call_ended': {
                this.finishCall(String(event.call_id ?? ''), String(event.reason ?? ''));
                break;
            }
            case 'remote_session_requested':
                this.remote = (event.session as RemoteSession | undefined) ?? this.remote;
                if (this.remote !== null) {
                    this.remote = { ...this.remote, state: 'requested', indicator: false };
                }
                break;
            case 'remote_session_started':
                this.remote = (event.session as RemoteSession | undefined) ?? this.remote;
                if (this.remote !== null) {
                    this.remote = { ...this.remote, state: 'active', indicator: true };
                }
                break;
            case 'remote_session_ended': {
                const ended = this.remote;
                if (ended !== null) {
                    this.remoteOutcome = {
                        userId: ended.user_id,
                        role: ended.role,
                        reason: String(event.reason ?? 'ended'),
                        endedAt: Date.now(),
                    };
                }
                this.remote = null;
                this.remoteInputPending = false;
                break;
            }
            case 'remote_indicator_changed':
                if (this.remote !== null) {
                    this.remote = { ...this.remote, indicator: Boolean(event.visible) };
                }
                break;
            case 'remote_input_changed':
                if (
                    this.remote !== null &&
                    this.remote.session_id === String(event.session_id ?? '')
                ) {
                    this.remote = {
                        ...this.remote,
                        input_allowed: Boolean(event.allowed),
                    };
                }
                break;
            case 'watch_party_requested':
            case 'watch_party_changed': {
                const party = event.party as WatchParty;
                this.adoptParty(party);
                this.prefetchYouTubeStream(party.video_id);
                break;
            }
            case 'watch_party_ended': {
                const ended = this.party;
                if (ended !== null) {
                    this.partyOutcome = {
                        partyId: ended.party_id,
                        hostId: ended.host_id,
                        reason: String(event.reason ?? 'ended'),
                        endedAt: Date.now(),
                    };
                }
                this.party = null;
                this.closePlayer();
                break;
            }
            case 'settings_changed':
                if (this.settingsDirty || this.liveSettingTimers.size > 0) {
                    break;
                }
                void this.loadSettings();
                break;
            case 'transfer_progress':
                this.setTransfer(
                    String(event.hash ?? ''),
                    Number(event.bytes ?? 0),
                    Number(event.total ?? 0),
                );
                break;
            case 'transfer_done':
                this.finishTransfer(String(event.hash ?? ''), '');
                break;
            case 'transfer_failed':
                this.finishTransfer(
                    String(event.hash ?? ''),
                    String(event.reason ?? 'transfer failed'),
                );
                break;
            case 'error':
                this.notice = String(event.message ?? 'bridge error');
                break;
            default:
                break;
        }
    }

    private receiveMessage(message: Message): void {
        const normalized = this.normalizeMessage(message);
        if (this.route.kind === 'chat' && this.route.id === normalized.user_id) {
            this.appendMessage(normalized);
            if (!normalized.local_sender && !this.readMessageIds.has(normalized.id)) {
                void this.markMessageRead(normalized.conversation_id, [normalized.id]);
            }
        }
        this.updateSummary(normalized);
        const videoId = firstVideoId(normalized.body);
        if (
            videoId !== null &&
            this.preferences.autoplay &&
            !this.preferences.ignoreIncoming &&
            this.party === null
        ) {
            this.openPlayer(videoId, 0, true);
        }
        if (this.route.kind !== 'chat' || this.route.id !== normalized.user_id) {
            if (normalized.local_sender) {
                this.scheduleRefresh();
                return;
            }
            this.pushToast({
                id: normalized.id,
                userId: normalized.user_id,
                name: this.userName(normalized.user_id),
                body: normalized.body,
                sentAt: normalized.sent_at || normalized.received_at,
            });
        }
        this.scheduleRefresh();
    }

    private finishRemote(reason: string): void {
        const ended = this.remote;
        this.remote = null;
        this.remoteInputPending = false;
        if (ended === null) {
            return;
        }
        this.remoteOutcome = {
            userId: ended.user_id,
            role: ended.role,
            reason,
            endedAt: Date.now(),
        };
    }

    private finishCall(callId: string, reason: string): void {
        const finished = this.call;
        this.call = null;
        this.syncCallLevelPolling();
        if (finished === null) {
            void this.loadCallLogs();
            return;
        }
        const endedAt = Date.now();
        const duration =
            finished.connected_at > 0
                ? Math.max(0, Math.floor((endedAt - finished.connected_at) / 1_000))
                : 0;
        this.callLogs = [
            {
                id: callId,
                user_id: finished.user_id,
                direction: finished.direction,
                started_at: finished.started_at,
                connected_at: finished.connected_at,
                ended_at: endedAt,
                duration_secs: duration,
                reason,
            },
            ...this.callLogs,
        ];
        this.pushToast({
            id: `call-${callId}`,
            userId: finished.user_id,
            name: this.userName(finished.user_id),
            body: `Voice call ended · ${sessionClock(duration)}`,
            sentAt: endedAt,
        });
        void this.loadCallLogs();
    }

    userName(userId: string): string {
        const identity = this.bootstrap?.identity;
        if (identity?.node_id === userId) {
            return identity.display_name.trim() || 'You';
        }
        const user = this.users.find((item) => item.id === userId);
        if (user === undefined) {
            return userId.slice(0, 8);
        }
        return user.display_name.trim() || user.id.slice(0, 8);
    }

    isSelf(userId: string): boolean {
        return this.bootstrap?.identity.node_id === userId;
    }

    participantAvatar(userId: string): string {
        if (this.isSelf(userId)) {
            return this.selfAvatarUrl || this.avatarUrl(this.bootstrap?.identity.fingerprint ?? '');
        }
        return this.avatarUrl(this.userById(userId)?.fingerprint ?? '');
    }

    userById(userId: string): User | null {
        return this.users.find((user) => user.id === userId) ?? null;
    }

    userVolume(userId: string): number {
        return clampVolume(this.preferences.userVolumes[userId] ?? DEFAULT_USER_VOLUME);
    }

    audioInputVolume(): number {
        return settingVolume(this.settings['audio.input_volume']);
    }

    audioOutputVolume(): number {
        return settingVolume(this.settings['audio.output_volume']);
    }

    messageOutputVolume(senderId: string): number {
        const userLevel = this.isSelf(senderId) ? DEFAULT_USER_VOLUME : this.userVolume(senderId);
        return (this.audioOutputVolume() / MAX_VOLUME) * (userLevel / MAX_VOLUME);
    }

    private async syncCallVolume(): Promise<void> {
        const call = this.call;
        if (this.client === null || call === null || call.state !== 'in_call') {
            return;
        }
        const volumeResult = await this.client.request('set_call_volume', {
            call_id: call.id,
            volume: this.userVolume(call.user_id),
        });
        if (!volumeResult.ok) {
            this.notice = volumeResult.error.message;
        }
    }
}

function settingVolume(value: string | undefined): number {
    if (value === undefined) {
        return MAX_VOLUME;
    }
    const parsed = Number(value);
    return Number.isFinite(parsed) ? clampVolume(parsed) : MAX_VOLUME;
}

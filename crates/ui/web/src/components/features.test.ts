import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
    Bootstrap,
    CallLog,
    Message,
    StorageUsage,
    User,
    UserDetails,
    WatchParty,
} from '../lib';
import { AppState, dateLabel } from '../lib';
import AppLifecycleView from './AppLifecycleView.svelte';
import CallView from './CallView.svelte';
import ChatView from './ChatView.svelte';
import ProfileView from './ProfileView.svelte';
import RemoteView from './RemoteView.svelte';
import SettingsView from './SettingsView.svelte';
import UsersRail from './UsersRail.svelte';
import WatchPartyView from './WatchPartyView.svelte';

const CREATED_AT = new Date(2025, 0, 2, 12, 0, 0).getTime();
const USER: User = {
    id: 'user-1',
    fingerprint: 'b371f09a2c5d4e6f8190',
    display_name: 'MIKA-PC',
    avatar_hash: '',
    policy: 'accept',
    trusted_at: CREATED_AT,
    last_seen: CREATED_AT,
    connected: true,
};
const DETAILS: UserDetails = {
    user: USER,
    addresses: '192.168.1.24',
    port: 45871,
    source: 'mdns',
    mac: 'not exposed',
    os: 'Linux',
    desktop: 'GNOME',
    display_server: 'Wayland',
    app_version: '0.1.1',
};
const STORAGE: StorageUsage = {
    message_count: 4,
    attachment_count: 2,
    attachment_bytes: 2048,
    database_bytes: 4096,
};

function makeApp(devBuild = false): AppState {
    const app = new AppState();
    const bootstrap: Bootstrap = {
        identity: {
            node_id: 'node-1',
            fingerprint: USER.fingerprint,
            display_name: USER.display_name,
            avatar_hash: '',
            created_at: CREATED_AT,
        },
        status: {
            transport_state: 'idle',
            active_sessions: 0,
            outbox_depth: 0,
            user_count: 1,
        },
        network: {
            state: 'no_peers',
            confidence: 'unknown',
            hint: 'No peers replied.',
            interfaces: 'eth0: 192.168.1.20',
            interface_count: 1,
            transport_port: 45871,
            transport_bound: true,
            mdns_active: true,
            broadcast_active: true,
            candidate_count: 0,
            active_sessions: 0,
            beacons_sent: 0,
            beacons_received: 0,
            beacon_send_errors: 0,
            dropped_packets: 0,
            last_beacon_sent_at: 0,
            last_beacon_received_at: 0,
            last_candidate_at: 0,
            last_session_at: 0,
            last_failure_at: 0,
            transport_failures: 0,
        },
        settings: {
            'identity.display_name': USER.display_name,
            'appearance.color_scheme': 'system',
            'discovery.enabled': 'true',
            'discovery.mdns': 'true',
            'discovery.broadcast': 'true',
            'discovery.beacon_interval_secs': '5',
        },
        appearance: 'system',
        autostart: false,
        version: '0.1.1',
        dev_build: devBuild,
        online: true,
    };
    app.bootstrap = bootstrap;
    app.network = bootstrap.network;
    app.settings = { ...bootstrap.settings };
    app.settingsDraft = { ...bootstrap.settings };
    app.users = [USER];
    app.storageUsage = STORAGE;
    app.route = { kind: 'chat', id: USER.id };
    return app;
}

function textMessage(id: string, body: string, sentAt: number): Message {
    return {
        id,
        conversation_id: 'conversation-1',
        user_id: USER.id,
        sender_id: USER.id,
        local_sender: false,
        seq: 1,
        sent_at: sentAt,
        received_at: sentAt,
        body,
        reply_to: '',
        attachment_hash: '',
        attachment_name: '',
        attachment_size: 0,
        attachment_mime: '',
        attachment_path: '',
        state: 'read',
        edited_at: 0,
        deleted_at: 0,
        notify: true,
    };
}

function audioMessage(): Message {
    return {
        id: 'message-audio',
        conversation_id: 'conversation-1',
        user_id: USER.id,
        sender_id: USER.id,
        local_sender: false,
        seq: 1,
        sent_at: CREATED_AT,
        received_at: CREATED_AT,
        body: '',
        reply_to: '',
        attachment_hash: 'hash-1',
        attachment_name: 'voice.webm',
        attachment_size: 2048,
        attachment_mime: 'audio/webm',
        attachment_path: '',
        state: 'read',
        edited_at: 0,
        deleted_at: 0,
        notify: true,
    };
}

class FakeMediaRecorder {
    static isTypeSupported = vi.fn((type: string) => type.includes('webm'));

    state = 'inactive';
    mimeType: string;
    ondataavailable: ((event: { data: Blob }) => void) | null = null;
    onstop: (() => void) | null = null;

    constructor(
        readonly stream: MediaStream,
        options?: { mimeType?: string },
    ) {
        this.mimeType = options?.mimeType ?? 'audio/webm';
    }

    start(): void {
        this.state = 'recording';
    }

    stop(): void {
        this.state = 'inactive';
        this.ondataavailable?.({ data: new Blob(['voice'], { type: this.mimeType }) });
        this.onstop?.();
    }
}

function stubDecodableAudio(): void {
    const channel = new Float32Array(600).fill(0.5);
    const decodeAudioData = vi.fn(async () => ({
        duration: 6,
        getChannelData: () => channel,
    }));
    const close = vi.fn(async () => undefined);
    class FakeAudioContext {
        decodeAudioData = decodeAudioData;
        close = close;
    }
    vi.stubGlobal('AudioContext', FakeAudioContext);
    class FakeResizeObserver {
        constructor(private callback: ResizeObserverCallback) {}
        observe(): void {
            this.callback(
                [{ contentRect: { width: 180 } }] as unknown as ResizeObserverEntry[],
                this as unknown as ResizeObserver,
            );
        }
        disconnect(): void {}
        unobserve(): void {}
    }
    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
    vi.stubGlobal(
        'fetch',
        vi.fn(async () => new Response(new Uint8Array([1, 2, 3]))),
    );
    vi.spyOn(window.HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined);
    vi.spyOn(window.HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
}

describe('feature surfaces', () => {
    afterEach(() => {
        vi.useRealTimers();
    });
    it('changes appearance from Settings and applies it immediately', async () => {
        const themeColor = document.createElement('meta');
        themeColor.name = 'theme-color';
        document.head.append(themeColor);

        const app = makeApp();
        app.route = { kind: 'settings', section: 'user' };
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'dark' }));
        expect(app.settingsDraft['appearance.color_scheme']).toBe('dark');
        expect(document.documentElement.dataset.theme).toBe('dark');
        expect(themeColor.content).toBe('#182b2f');

        await fireEvent.click(screen.getByRole('button', { name: 'light' }));
        expect(app.settingsDraft['appearance.color_scheme']).toBe('light');
        expect(document.documentElement.dataset.theme).toBe('light');
        expect(themeColor.content).toBe('#f4efe2');

        await fireEvent.click(screen.getByRole('button', { name: 'system' }));
        expect(app.settingsDraft['appearance.color_scheme']).toBe('system');
        expect(document.documentElement.dataset.theme).toBeUndefined();
        expect(themeColor.content).toBe('#182b2f');
    });

    it('edits message sounds from Settings', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'notifications' };
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Message sounds' }));
        expect(app.settingsDraft['notifications.message_sounds']).toBe('false');
    });

    it('edits discovery controls from Settings', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'about' };
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Discovery' }));
        expect(app.settingsDraft['discovery.enabled']).toBe('false');

        await fireEvent.click(screen.getByRole('button', { name: 'mDNS discovery' }));
        expect(app.settingsDraft['discovery.mdns']).toBe('false');

        await fireEvent.click(screen.getByRole('button', { name: 'Broadcast discovery' }));
        expect(app.settingsDraft['discovery.broadcast']).toBe('false');

        const interval = screen.getByDisplayValue('5');
        await fireEvent.input(interval, { target: { value: '30' } });
        expect(app.settingsDraft['discovery.beacon_interval_secs']).toBe('30');
    });

    it('opens the audio pane from the settings rail', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'calls' };
        render(SettingsView, { app });

        const audioLink = screen.getByRole('link', { name: 'Audio' });
        expect(audioLink.getAttribute('href')).toBe('#/settings/audio');
        await fireEvent.click(audioLink);

        expect(app.route).toEqual({ kind: 'settings', section: 'audio' });
        expect(screen.getByLabelText('Input volume')).toBeTruthy();
        expect(screen.getByLabelText('Output volume')).toBeTruthy();

        await fireEvent.click(screen.getByRole('link', { name: 'Calls' }));
        expect(app.route).toEqual({ kind: 'settings', section: 'calls' });
        expect(screen.getByText('Ringing timeout')).toBeTruthy();

        await fireEvent.click(screen.getByRole('link', { name: 'Audio' }));
        expect(app.route).toEqual({ kind: 'settings', section: 'audio' });
        expect(screen.getByLabelText('Input volume')).toBeTruthy();
        expect(screen.getByLabelText('Output volume')).toBeTruthy();
    });

    it('deduplicates repeated audio device names', () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'audio' };
        app.audioDevices = [
            { id: 'HDA Intel PCH', name: 'HDA Intel PCH', kind: 'input' },
            { id: 'HDA Intel PCH', name: 'HDA Intel PCH', kind: 'input' },
            { id: 'PipeWire', name: 'PipeWire', kind: 'input' },
            { id: 'HDA Intel PCH', name: 'HDA Intel PCH', kind: 'output' },
            { id: 'HDMI', name: 'HDMI', kind: 'output' },
            { id: 'HDMI', name: 'HDMI', kind: 'output' },
        ];
        render(SettingsView, { app });

        const microphone = screen.getByRole('combobox', { name: 'Microphone' });
        expect(within(microphone).getAllByRole('option')).toHaveLength(3);
        const speakers = screen.getByRole('combobox', { name: 'Speakers' });
        expect(within(speakers).getAllByRole('option')).toHaveLength(3);
    });

    it('edits audio volumes from Settings', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'audio' };
        render(SettingsView, { app });

        const inputVolume = screen.getByLabelText('Input volume');
        await fireEvent.input(inputVolume, { target: { value: '72' } });
        expect(app.settingsDraft['audio.input_volume']).toBe('72');

        const outputVolume = screen.getByLabelText('Output volume');
        await fireEvent.input(outputVolume, { target: { value: '64' } });
        expect(app.settingsDraft['audio.output_volume']).toBe('64');
    });

    it('shows LAN access state and firewall ports in Settings', () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'about' };
        render(SettingsView, { app });

        expect(screen.getByText('LAN access')).toBeTruthy();
        expect(screen.getByText('No peers found')).toBeTruthy();
        expect(screen.getByText('5353 · 45870 · 45871')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Check LAN access' })).toBeTruthy();
    });

    it('renders identity age and real storage usage', () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'user' };
        const { unmount } = render(SettingsView, { app });
        expect(screen.getByText(dateLabel(CREATED_AT))).toBeTruthy();
        unmount();

        app.route = { kind: 'settings', section: 'storage' };
        render(SettingsView, { app });
        expect(screen.getByText(/4 messages/)).toBeTruthy();
        expect(screen.getByText(/2 attachments/)).toBeTruthy();
    });

    it('asks before clearing all history', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'storage' };
        const clear = vi.spyOn(app, 'clearHistory').mockResolvedValue(undefined);
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        expect(screen.getByRole('alertdialog')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Clear history' }));
        expect(clear).toHaveBeenCalledWith('');
    });

    it('does not clear history when the confirmation is rejected', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'storage' };
        const clear = vi.spyOn(app, 'clearHistory').mockResolvedValue(undefined);
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
        expect(clear).not.toHaveBeenCalled();
    });

    it('clears one conversation from the profile', async () => {
        const app = makeApp();
        app.route = { kind: 'profile', id: USER.id };
        app.details = DETAILS;
        const clear = vi.spyOn(app, 'clearHistory').mockResolvedValue(undefined);
        render(ProfileView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        await fireEvent.click(screen.getByRole('button', { name: 'Clear history' }));
        expect(clear).toHaveBeenCalledWith(USER.id);
    });

    it('stores per-user volume from the profile', async () => {
        const app = makeApp();
        app.route = { kind: 'profile', id: USER.id };
        app.details = DETAILS;
        render(ProfileView, { app });

        const volume = screen.getByLabelText('User volume');
        await fireEvent.input(volume, { target: { value: '70' } });
        await fireEvent.change(volume, { target: { value: '70' } });
        expect(app.userVolume(USER.id)).toBe(70);
        expect(screen.getByText('70%')).toBeTruthy();
    });

    it('tracks dirty and saved settings state', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'about' };
        const save = vi.spyOn(app, 'saveSettings');
        render(SettingsView, { app });

        expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty(
            'disabled',
            true,
        );
        await fireEvent.click(screen.getByRole('button', { name: 'Discovery' }));
        expect(screen.getByText('Unsaved changes')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Save changes' })).toHaveProperty(
            'disabled',
            false,
        );
        await fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));
        expect(save).toHaveBeenCalledTimes(1);
        await waitFor(() => expect(screen.getByText('All changes saved')).toBeTruthy());
    });

    it('shows loading, empty, and error states in the user rail', async () => {
        const app = makeApp();
        app.usersLoad = 'loading';
        const loading = render(UsersRail, { app });
        expect(screen.getByRole('status', { name: 'Loading people' })).toBeTruthy();
        loading.unmount();

        app.usersLoad = 'empty';
        app.users = [];
        render(UsersRail, { app });
        expect(screen.getByText('No one here yet')).toBeTruthy();
    });

    it('shows a retry banner when the user list fails', async () => {
        const app = makeApp();
        app.usersLoad = 'error';
        app.usersError = 'daemon unavailable';
        const retry = vi.spyOn(app, 'retryRefresh').mockResolvedValue(undefined);
        render(UsersRail, { app });

        expect(screen.getByText('People did not load')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
        expect(retry).toHaveBeenCalledTimes(1);
    });

    it('expands from collapsed search and focuses the search field', async () => {
        const app = makeApp();
        app.users = [USER];
        app.collapsed = true;
        render(UsersRail, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Search users' }));

        expect(app.collapsed).toBe(false);
        const search = screen.getByLabelText('Search users');
        await waitFor(() => expect(document.activeElement).toBe(search));
    });

    it('expands from the collapsed rail with the slash shortcut', async () => {
        const app = makeApp();
        app.users = [USER];
        app.collapsed = true;
        render(UsersRail, { app });

        await fireEvent.keyDown(window, { key: '/' });

        expect(app.collapsed).toBe(false);
        const search = screen.getByLabelText('Search users');
        await waitFor(() => expect(document.activeElement).toBe(search));
    });

    it('keeps the logo below the collapsed rail actions', () => {
        const app = makeApp();
        app.collapsed = true;
        render(UsersRail, { app });

        const settings = screen.getByRole('button', { name: 'Settings' });
        const logo = screen.getByRole('link', { name: 'TiConnect' });
        expect(
            settings.compareDocumentPosition(logo) & Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
    });

    it('shows the version beside the logo in the expanded rail only', () => {
        const app = makeApp();
        const expanded = render(UsersRail, { app });

        const version = screen.getByText('0.1.1');
        const logo = screen.getByRole('img', { name: 'TiConnect' });
        expect(
            logo.compareDocumentPosition(version) & Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
        expect(screen.queryByText('· DEV')).toBeNull();
        expanded.unmount();

        app.collapsed = true;
        render(UsersRail, { app });

        expect(screen.queryByText('0.1.1')).toBeNull();
    });

    it('marks development builds beside the version', () => {
        const app = makeApp(true);
        render(UsersRail, { app });

        expect(screen.getByText('0.1.1')).toBeTruthy();
        expect(screen.getByText('· DEV')).toBeTruthy();
    });

    it('shows message timeline loading and failure states', async () => {
        const app = makeApp();
        app.messagesLoad = 'loading';
        const loading = render(ChatView, { app });
        expect(screen.getByRole('status', { name: 'Loading messages' })).toBeTruthy();
        loading.unmount();

        app.messagesLoad = 'error';
        app.messagesError = 'conversation unavailable';
        const retry = vi.spyOn(app, 'loadMessages').mockResolvedValue(undefined);
        render(ChatView, { app });
        expect(screen.getByText('Messages did not load')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
        expect(retry).toHaveBeenCalledWith(USER.id);
    });

    it('shows the connecting call state before the remote rings', () => {
        const app = makeApp();
        app.route = { kind: 'call', id: USER.id };
        app.call = {
            id: 'call-1',
            user_id: USER.id,
            direction: 'outgoing',
            state: 'connecting',
            started_at: CREATED_AT,
            connected_at: 0,
            input_muted: false,
            output_muted: false,
        };
        render(CallView, { app });

        expect(screen.getByText('Connecting — waiting for answer')).toBeTruthy();
        expect(screen.getByText(/Voice call · Connecting/)).toBeTruthy();
    });

    it('shows remote session outcomes and allows another request', async () => {
        const app = makeApp();
        app.route = { kind: 'remote', id: USER.id };
        app.remoteOutcome = {
            userId: USER.id,
            role: 'viewer',
            reason: 'denied',
            endedAt: CREATED_AT,
        };
        const request = vi.spyOn(app, 'requestRemote').mockResolvedValue(undefined);
        render(RemoteView, { app });

        expect(screen.getByText('Request declined')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Request again' }));
        expect(request).toHaveBeenCalledWith(USER.id);
    });

    it('updates remote input policy from the host controls', async () => {
        const app = makeApp();
        app.route = { kind: 'remote', id: USER.id };
        app.remote = {
            session_id: 'session-1',
            user_id: USER.id,
            fingerprint: USER.fingerprint,
            role: 'host',
            state: 'active',
            indicator: true,
            input_allowed: true,
            expires_at: CREATED_AT + 300_000,
        };
        const setInput = vi.spyOn(app, 'setRemoteInput').mockResolvedValue(undefined);
        render(RemoteView, { app });

        const toggle = screen.getByRole('button', { name: 'Allow pointer and keyboard' });
        expect(toggle.getAttribute('aria-pressed')).toBe('true');
        await fireEvent.click(toggle);
        expect(setInput).toHaveBeenCalledWith(false);
    });

    it('shows the offline lifecycle screen with retry', async () => {
        const retry = vi.fn();
        render(AppLifecycleView, {
            mode: 'offline',
            errorMessage: 'daemon unavailable',
            reconnectAttempt: 2,
            onretry: retry,
        });

        expect(screen.getByText('Cannot reach TiConnect')).toBeTruthy();
        expect(screen.getByText('Retry attempt 2')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Retry connection' }));
        expect(retry).toHaveBeenCalledTimes(1);
    });

    it('blocks sending when offline, blocked, or over the body limit', () => {
        const app = makeApp();
        app.connection = 'offline';
        const offline = render(ChatView, { app });
        expect(screen.getByText('Daemon offline — messages cannot be sent.')).toBeTruthy();
        expect(screen.queryByRole('button', { name: 'Send message' })).toBeNull();
        expect(screen.getByRole('button', { name: 'Record voice message' })).toHaveProperty(
            'disabled',
            true,
        );
        offline.unmount();

        app.connection = 'online';
        app.users = [{ ...USER, policy: 'ignore' }];
        const blocked = render(ChatView, { app });
        expect(
            screen.getByText(
                'This user is blocked. Unblock them from their profile to send messages.',
            ),
        ).toBeTruthy();
        blocked.unmount();

        app.users = [USER];
        app.draft = 'a'.repeat(32_769);
        render(ChatView, { app });
        expect(screen.getByText('Message is too long')).toBeTruthy();
        expect(screen.getByText(/32769 of 32768 bytes/)).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Send message' })).toHaveProperty(
            'disabled',
            true,
        );
    });

    it('edits and deletes local text messages', async () => {
        const app = makeApp();
        const message = { ...textMessage('message-edit', 'original', Date.now()) };
        message.local_sender = true;
        message.state = 'sent';
        app.messages = [message];
        app.messagesLoad = 'ready';
        const edit = vi.spyOn(app, 'editMessage').mockResolvedValue(true);
        const remove = vi.spyOn(app, 'deleteMessage').mockResolvedValue(true);
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
        const field = screen.getByLabelText('Message text');
        expect(field.closest('article')?.className).toMatch(/(?:^|\s)w-full(?:\s|$)/);
        await fireEvent.input(field, { target: { value: 'updated text' } });
        await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
        expect(edit).toHaveBeenCalledWith('message-edit', 'updated text');

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
        await fireEvent.input(screen.getByLabelText('Message text'), {
            target: { value: 'saved with enter' },
        });
        await fireEvent.keyDown(screen.getByLabelText('Message text'), { key: 'Enter' });
        await waitFor(() => {
            expect(edit).toHaveBeenCalledWith('message-edit', 'saved with enter');
        });

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
        await fireEvent.keyDown(screen.getByLabelText('Message text'), { key: 'Escape' });
        expect(screen.queryByLabelText('Message text')).toBeNull();

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Delete' }));
        expect(screen.getByRole('alertdialog')).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Delete message' }));
        expect(remove).toHaveBeenCalledWith('message-edit');
    });

    it('closes the message menu on outside click and on Escape', async () => {
        const app = makeApp();
        const message = { ...textMessage('message-menu', 'original', Date.now()) };
        message.local_sender = true;
        message.state = 'sent';
        app.messages = [message];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        expect(screen.getByRole('menuitem', { name: 'Edit' })).toBeTruthy();
        expect(screen.getByRole('menu').classList.contains('w-40')).toBe(true);

        await fireEvent.click(screen.getByText('original'));
        expect(screen.queryByRole('menuitem', { name: 'Edit' })).toBeNull();

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        expect(screen.getByRole('menuitem', { name: 'Edit' })).toBeTruthy();

        await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
        expect(screen.queryByRole('menuitem', { name: 'Edit' })).toBeNull();
    });

    it('hides the action button after closing the menu with it', async () => {
        const app = makeApp();
        const message = { ...textMessage('message-menu-close', 'original', Date.now()) };
        message.local_sender = true;
        message.state = 'sent';
        app.messages = [message];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        const actions = screen.getByRole('button', { name: 'Message actions' });
        await fireEvent.click(actions);
        expect(actions.getAttribute('aria-expanded')).toBe('true');
        expect(actions.classList.contains('opacity-100')).toBe(true);

        await fireEvent.click(actions);
        expect(screen.queryByRole('menuitem', { name: 'Edit' })).toBeNull();
        expect(actions.getAttribute('aria-expanded')).toBe('false');
        expect(actions.classList.contains('opacity-100')).toBe(false);
    });

    it('hides the edit action once the five minute window passes', async () => {
        const app = makeApp();
        const message = {
            ...textMessage('message-stale', 'original', Date.now() - 6 * 60 * 1000),
        };
        message.local_sender = true;
        message.state = 'sent';
        app.messages = [message];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        expect(screen.getByRole('menuitem', { name: 'Reply' })).toBeTruthy();
        expect(screen.queryByRole('menuitem', { name: 'Edit' })).toBeNull();
    });

    it('replies to a message and jumps to the original', async () => {
        const app = makeApp();
        const original = { ...textMessage('message-original', 'quoted text', Date.now()) };
        original.local_sender = false;
        const replier = { ...textMessage('message-reply', 'answer', Date.now()) };
        replier.local_sender = true;
        replier.state = 'sent';
        replier.reply_to = original.id;
        app.messages = [original, replier];
        app.messagesLoad = 'ready';
        const scrollIntoView = vi.fn();
        window.HTMLElement.prototype.scrollIntoView = scrollIntoView;
        render(ChatView, { app });

        expect(screen.getAllByText('quoted text').length).toBeGreaterThan(0);
        vi.useFakeTimers();
        await fireEvent.click(screen.getByRole('button', { name: 'Jump to original message' }));
        expect(scrollIntoView).toHaveBeenCalled();
        const target = document.getElementById('chat-message-message-original');
        expect(target?.className).toContain('reply-flash');

        await vi.advanceTimersByTimeAsync(1700);
        expect(document.getElementById('chat-message-message-original')?.className).not.toContain(
            'reply-flash',
        );

        await fireEvent.click(
            screen.getAllByRole('button', { name: 'Message actions' })[1] as HTMLElement,
        );
        await fireEvent.click(screen.getByRole('menuitem', { name: 'Reply' }));
        expect(app.replyToId).toBe(replier.id);
        expect(screen.getByText(/Replying to/)).toBeTruthy();
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel reply' }));
        expect(app.replyToId).toBe('');
    });

    it('offers only reply on remote messages', async () => {
        const app = makeApp();
        const remote = { ...textMessage('message-remote', 'from a peer', Date.now()) };
        remote.local_sender = false;
        app.messages = [remote];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Message actions' }));
        expect(screen.getByRole('menuitem', { name: 'Reply' })).toBeTruthy();
        expect(screen.queryByRole('menuitem', { name: 'Edit' })).toBeNull();
        expect(screen.queryByRole('menuitem', { name: 'Delete' })).toBeNull();

        await fireEvent.click(screen.getByRole('menuitem', { name: 'Reply' }));
        expect(app.replyToId).toBe(remote.id);
    });

    it('mirrors message actions by sender', () => {
        const app = makeApp();
        const remote = { ...textMessage('message-remote-side', 'from a peer', CREATED_AT) };
        const local = {
            ...textMessage('message-local-side', 'from me', CREATED_AT + 1),
        };
        local.local_sender = true;
        local.state = 'sent';
        app.messages = [remote, local];
        app.messagesLoad = 'ready';
        const { container } = render(ChatView, { app });

        const remoteArticle = container.querySelector('#chat-message-message-remote-side');
        const localArticle = container.querySelector('#chat-message-message-local-side');
        expect(remoteArticle?.hasAttribute('data-local')).toBe(false);
        expect(remoteArticle?.classList.contains('flex-row-reverse')).toBe(true);
        expect(localArticle?.hasAttribute('data-local')).toBe(true);
        expect(localArticle?.classList.contains('data-local:flex-row')).toBe(true);
    });

    it('shows no actions on deleted remote messages', async () => {
        const app = makeApp();
        const remote = { ...textMessage('message-remote-deleted', 'gone', Date.now()) };
        remote.local_sender = false;
        remote.deleted_at = Date.now();
        app.messages = [remote];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        expect(screen.queryByRole('button', { name: 'Message actions' })).toBeNull();
    });

    it('renders edited markers and deleted tombstones', () => {
        const app = makeApp();
        const edited = { ...textMessage('message-edited', 'updated', CREATED_AT) };
        edited.edited_at = CREATED_AT;
        const deleted = { ...textMessage('message-deleted', 'hidden body', CREATED_AT) };
        deleted.deleted_at = CREATED_AT;
        app.messages = [edited, deleted];
        app.messagesLoad = 'ready';
        render(ChatView, { app });

        expect(screen.getByText('· edited')).toBeTruthy();
        expect(screen.getByText('Message deleted')).toBeTruthy();
        expect(screen.queryByText('hidden body')).toBeNull();
    });

    it('offers retry for failed local messages', async () => {
        const app = makeApp();
        const failed = { ...textMessage('message-failed', 'try again', CREATED_AT) };
        failed.local_sender = true;
        failed.state = 'failed';
        app.messages = [failed];
        app.messagesLoad = 'ready';
        const retry = vi.spyOn(app, 'retryMessage').mockResolvedValue(undefined);
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
        expect(retry).toHaveBeenCalledWith('message-failed');
    });

    it('keeps forming recording bars while the input holds steady', async () => {
        vi.useFakeTimers();
        const app = makeApp();
        const stream = {
            getTracks: () => [{ stop: vi.fn() }],
        } as unknown as MediaStream;
        vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
        vi.stubGlobal('navigator', {
            mediaDevices: { getUserMedia: vi.fn(async () => stream) },
        });
        const getByteTimeDomainData = vi.fn((target: Uint8Array) => {
            target.fill(200);
        });
        const analyser = { fftSize: 0, getByteTimeDomainData };
        const connect = vi.fn();
        class FakeAudioContext {
            state: AudioContextState = 'running';
            resume = vi.fn(async () => undefined);
            close = vi.fn(async () => undefined);
            createMediaStreamSource = vi.fn(() => ({ connect }));
            createGain = vi.fn(() => ({ connect, gain: { value: 1 } }));
            createMediaStreamDestination = vi.fn(() => ({ connect, stream }));
            createAnalyser = vi.fn(() => analyser);
        }
        vi.stubGlobal('AudioContext', FakeAudioContext);
        class FakeResizeObserver {
            constructor(private callback: ResizeObserverCallback) {}
            observe(): void {
                this.callback(
                    [{ contentRect: { width: 300 } }] as unknown as ResizeObserverEntry[],
                    this as unknown as ResizeObserver,
                );
            }
            disconnect(): void {}
            unobserve(): void {}
        }
        vi.stubGlobal('ResizeObserver', FakeResizeObserver);
        const { container } = render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Record voice message' }));
        await act(async () => {
            await vi.advanceTimersByTimeAsync(450);
        });

        const bars = container.querySelectorAll('span[class*="w-[3px]"]');
        expect(bars.length).toBe(app.recordingWaveform.length);
        expect(bars.length).toBeGreaterThan(0);
        expect(bars.length).toBeLessThan(50);
    });

    it('starts, stops, and cancels voice recording from chat', async () => {
        const app = makeApp();
        const start = vi.spyOn(app, 'startRecording').mockResolvedValue(undefined);
        const stop = vi.spyOn(app, 'stopRecording').mockResolvedValue(undefined);
        const cancel = vi.spyOn(app, 'cancelRecording').mockImplementation(() => {});
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Record voice message' }));
        expect(start).toHaveBeenCalled();

        app.recording = true;
        app.recordingSeconds = 65;
        await waitFor(() => expect(screen.getByText('00:01:05')).toBeTruthy());
        await fireEvent.click(screen.getByRole('button', { name: 'Stop and send voice message' }));
        expect(stop).toHaveBeenCalled();
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel voice message' }));
        expect(cancel).toHaveBeenCalled();
    });

    it('loads a player-only bubble for audio attachments', async () => {
        stubDecodableAudio();
        const app = makeApp();
        app.messages = [audioMessage()];
        const url = vi.spyOn(app, 'attachmentUrl').mockResolvedValue('blob:voice');
        render(ChatView, { app });

        await waitFor(() => expect(screen.getByText('00:06')).toBeTruthy());
        expect(screen.getByRole('button', { name: 'Play voice message' })).toBeTruthy();
        expect(screen.queryByText('voice.webm')).toBeNull();
    });

    it('interleaves call events with messages by time', async () => {
        const app = makeApp();
        app.messages = [
            textMessage('message-before', 'Before the call', CREATED_AT),
            textMessage('message-after', 'After the call', CREATED_AT + 7_200_000),
        ];
        const log: CallLog = {
            id: 'call-1',
            user_id: USER.id,
            direction: 'outgoing',
            started_at: CREATED_AT + 3_600_000,
            connected_at: CREATED_AT + 3_600_000,
            ended_at: CREATED_AT + 3_602_000,
            duration_secs: 2,
            reason: 'hangup',
        };
        app.callLogs = [log];
        render(ChatView, { app });

        const before = await screen.findByText('Before the call');
        const call = await screen.findByText(/Call ended · 00:00:02/);
        const after = await screen.findByText('After the call');
        expect(call.textContent).toMatch(/Call ended · 00:00:02 · \d{2}:\d{2}/);
        expect(
            before.compareDocumentPosition(call) & Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
        expect(call.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it('hides call events older than the loaded message window', async () => {
        const app = makeApp();
        app.messages = [textMessage('message-loaded', 'Loaded message', CREATED_AT + 3_600_000)];
        const log: CallLog = {
            id: 'call-old',
            user_id: USER.id,
            direction: 'outgoing',
            started_at: CREATED_AT,
            connected_at: CREATED_AT,
            ended_at: CREATED_AT + 2_000,
            duration_secs: 2,
            reason: 'hangup',
        };
        app.callLogs = [log];
        render(ChatView, { app });

        await screen.findByText('Loaded message');
        expect(screen.queryByText(/Call ended/)).toBeNull();
    });
});

describe('group watch party page', () => {
    it('selects people and starts a group party', async () => {
        const app = makeApp();
        const second: User = {
            ...USER,
            id: 'user-2',
            display_name: 'NINA-LAPTOP',
            fingerprint: '4e2c8a17d9b3f605e8a1',
        };
        app.users = [USER, second];
        app.route = { kind: 'watch-party' };
        app.party = null;
        render(WatchPartyView, { app });

        await fireEvent.input(screen.getByLabelText('YouTube link'), {
            target: { value: 'https://youtu.be/dQw4w9WgXcQ' },
        });
        await fireEvent.click(screen.getByLabelText('Select MIKA-PC'));
        await fireEvent.click(screen.getByLabelText('Select NINA-LAPTOP'));
        await fireEvent.click(screen.getByRole('button', { name: 'Start watch party' }));

        const started = app.party as WatchParty | null;
        expect(started?.video_id).toBe('dQw4w9WgXcQ');
        expect(started?.participants).toEqual(['node-1', 'user-1', 'user-2']);
        expect(app.route).toEqual({ kind: 'watch-party' });
    });

    it('filters people and keeps selections across queries', async () => {
        const app = makeApp();
        const nina: User = {
            ...USER,
            id: 'user-2',
            display_name: 'NINA-LAPTOP',
            fingerprint: '4e2c8a17d9b3f605e8a1',
        };
        const attic: User = {
            ...USER,
            id: 'user-3',
            display_name: 'ATTIC-NAS',
            fingerprint: '2b5d8c1e4f7a9d3b6e8c',
        };
        app.users = [USER, nina, attic];
        app.route = { kind: 'watch-party' };
        app.party = null;
        render(WatchPartyView, { app });

        await fireEvent.click(screen.getByLabelText('Select MIKA-PC'));
        const search = screen.getByLabelText('Search people');
        await fireEvent.input(search, { target: { value: 'nina' } });

        expect(screen.queryByLabelText('Select MIKA-PC')).toBeNull();
        expect(screen.getByLabelText('Select NINA-LAPTOP')).toBeTruthy();
        expect(screen.getByText('People · 1')).toBeTruthy();

        await fireEvent.input(search, { target: { value: 'nothing' } });
        expect(screen.getByText('No one matches “nothing”.')).toBeTruthy();

        await fireEvent.input(search, { target: { value: '' } });
        const mika = screen.getByLabelText('Select MIKA-PC') as HTMLInputElement;
        expect(mika.checked).toBe(true);
    });

    it('filters invitees in an active party', async () => {
        const app = makeApp();
        const nina: User = {
            ...USER,
            id: 'user-2',
            display_name: 'NINA-LAPTOP',
            fingerprint: '4e2c8a17d9b3f605e8a1',
        };
        app.users = [USER, nina];
        app.route = { kind: 'watch-party' };
        app.party = {
            party_id: 'party-1',
            host_id: 'node-1',
            video_id: 'dQw4w9WgXcQ',
            role: 'host',
            state: 'active',
            participants: ['node-1'],
            invited: [],
            position_ms: 0,
            playing: false,
            revision: 1,
            actor: 'node-1',
            updated_at: 1,
        };
        render(WatchPartyView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Invite people' }));
        const search = screen.getByLabelText('Search invitees');
        await fireEvent.input(search, { target: { value: 'nina' } });

        expect(screen.queryByLabelText('Invite MIKA-PC')).toBeNull();
        expect(screen.getByLabelText('Invite NINA-LAPTOP')).toBeTruthy();

        await fireEvent.input(search, { target: { value: 'nothing' } });
        expect(screen.getByText('No one matches “nothing”.')).toBeTruthy();
    });

    it('shows YouTube metadata before sending a watch request', async () => {
        const app = makeApp();
        app.route = { kind: 'watch-party' };
        app.party = null;
        vi.spyOn(app, 'youtubePreview').mockResolvedValue({
            video_id: 'dQw4w9WgXcQ',
            title: 'How we built TiConnect for the LAN',
            channel: 'TiConnect Labs',
            description: 'A walkthrough of LAN discovery and watch parties.',
            thumbnail_url: 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
            thumbnail_data: 'data:image/jpeg;base64,aGk=',
            duration_seconds: 213,
        });
        render(WatchPartyView, { app });

        await fireEvent.input(screen.getByLabelText('YouTube link'), {
            target: { value: 'https://youtu.be/dQw4w9WgXcQ' },
        });

        await waitFor(() =>
            expect(screen.getByText('How we built TiConnect for the LAN')).toBeTruthy(),
        );
        expect(screen.getByText('TiConnect Labs · YouTube')).toBeTruthy();
        expect(screen.getByText('A walkthrough of LAN discovery and watch parties.')).toBeTruthy();
        expect(screen.getByAltText('Video thumbnail').getAttribute('src')).toBe(
            'data:image/jpeg;base64,aGk=',
        );
    });

    it('keeps the link usable when metadata is unavailable', async () => {
        const app = makeApp();
        app.route = { kind: 'watch-party' };
        app.party = null;
        vi.spyOn(app, 'youtubePreview').mockResolvedValue(null);
        render(WatchPartyView, { app });

        await fireEvent.input(screen.getByLabelText('YouTube link'), {
            target: { value: 'https://youtu.be/dQw4w9WgXcQ' },
        });

        await waitFor(() =>
            expect(
                screen.getByText('Preview unavailable. You can still send the link.'),
            ).toBeTruthy(),
        );
        await fireEvent.error(screen.getByAltText('Video thumbnail'));
        expect(screen.getByText('Thumbnail unavailable')).toBeTruthy();
    });

    it('renders real participant counts instead of demo numbers', () => {
        const app = makeApp();
        app.users = [USER];
        app.route = { kind: 'watch-party' };
        app.party = {
            party_id: 'party-1',
            host_id: 'node-1',
            video_id: 'dQw4w9WgXcQ',
            role: 'host',
            state: 'active',
            participants: ['node-1', 'user-1'],
            invited: [],
            position_ms: 0,
            playing: false,
            revision: 1,
            actor: 'node-1',
            updated_at: 1,
        };
        render(WatchPartyView, { app });

        expect(screen.getByText('Watching · 2')).toBeTruthy();
        expect(screen.getAllByRole('button', { name: 'End party' }).length).toBeGreaterThan(0);
    });
});

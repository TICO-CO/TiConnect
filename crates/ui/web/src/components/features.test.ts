import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

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
import ChatView from './ChatView.svelte';
import ProfileView from './ProfileView.svelte';
import SettingsView from './SettingsView.svelte';
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
    app_version: '0.0.6',
};
const STORAGE: StorageUsage = {
    message_count: 4,
    attachment_count: 2,
    attachment_bytes: 2048,
    database_bytes: 4096,
};

function makeApp(): AppState {
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
        version: '0.0.6',
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
        attachment_hash: '',
        attachment_name: '',
        attachment_size: 0,
        attachment_mime: '',
        attachment_path: '',
        state: 'read',
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
        attachment_hash: 'hash-1',
        attachment_name: 'voice.webm',
        attachment_size: 2048,
        attachment_mime: 'audio/webm',
        attachment_path: '',
        state: 'read',
        notify: true,
    };
}

describe('feature surfaces', () => {
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
        const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true);
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        expect(confirm).toHaveBeenCalled();
        expect(clear).toHaveBeenCalledWith('');
    });

    it('does not clear history when the confirmation is rejected', async () => {
        const app = makeApp();
        app.route = { kind: 'settings', section: 'storage' };
        const clear = vi.spyOn(app, 'clearHistory').mockResolvedValue(undefined);
        vi.spyOn(window, 'confirm').mockReturnValue(false);
        render(SettingsView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        expect(clear).not.toHaveBeenCalled();
    });

    it('clears one conversation from the profile', async () => {
        const app = makeApp();
        app.route = { kind: 'profile', id: USER.id };
        app.details = DETAILS;
        const clear = vi.spyOn(app, 'clearHistory').mockResolvedValue(undefined);
        vi.spyOn(window, 'confirm').mockReturnValue(true);
        render(ProfileView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Clear local history' }));
        expect(clear).toHaveBeenCalledWith(USER.id);
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

    it('loads an inline player for audio attachments', async () => {
        const app = makeApp();
        app.messages = [audioMessage()];
        const url = vi.spyOn(app, 'attachmentUrl').mockResolvedValue('blob:voice');
        render(ChatView, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Play' }));
        await waitFor(() => expect(url).toHaveBeenCalledWith('hash-1'));
        await waitFor(() =>
            expect(document.querySelector('audio')?.getAttribute('src')).toBe('blob:voice'),
        );
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

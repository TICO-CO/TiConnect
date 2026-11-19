import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { Bootstrap, Message, StorageUsage, User, UserDetails } from '../lib';
import { AppState, dateLabel } from '../lib';
import ChatView from './ChatView.svelte';
import ProfileView from './ProfileView.svelte';
import SettingsView from './SettingsView.svelte';

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
    app.settings = { ...bootstrap.settings };
    app.settingsDraft = { ...bootstrap.settings };
    app.users = [USER];
    app.storageUsage = STORAGE;
    app.route = { kind: 'chat', id: USER.id };
    return app;
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
});

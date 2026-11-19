import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AppState } from './app-state.svelte';
import { BridgeClient } from './bridge';
import type { StorageUsage } from './types';

const USAGE: StorageUsage = {
    message_count: 4,
    attachment_count: 2,
    attachment_bytes: 2048,
    database_bytes: 4096,
};

interface FakeClient {
    request: ReturnType<typeof vi.fn>;
    uploadAttachment: ReturnType<typeof vi.fn>;
    downloadAttachment: ReturnType<typeof vi.fn>;
}

function makeApp(): { app: AppState; client: FakeClient } {
    const request = vi.fn(async (method: string) => {
        switch (method) {
            case 'list_users':
                return [[], null];
            case 'list_conversation_summaries':
                return [[], null];
            case 'watch_party':
                return [null, null];
            case 'storage_usage':
                return [USAGE, null];
            case 'clear_history':
                return [1, null];
            case 'list_settings':
                return [{ 'appearance.color_scheme': 'dark' }, null];
            case 'autostart_get':
                return [false, null];
            case 'settings_save':
                return [null, null];
            default:
                return [null, null];
        }
    });
    const uploadAttachment = vi.fn(async () => ['message-1', null]);
    const downloadAttachment = vi.fn(async () => ['blob:attachment', null]);
    const client: FakeClient = { request, uploadAttachment, downloadAttachment };
    const app = new AppState();
    (app as unknown as { client: BridgeClient }).client = client as unknown as BridgeClient;
    return { app, client };
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

describe('AppState feature behavior', () => {
    beforeEach(() => {
        vi.stubGlobal('window', globalThis);
    });

    it('records WebM and uploads it as an attachment', async () => {
        const { app } = makeApp();
        app.route = { kind: 'chat', id: 'user-1' };
        const stopTrack = vi.fn();
        const stream = {
            getTracks: () => [{ stop: stopTrack }],
        } as unknown as MediaStream;
        const getUserMedia = vi.fn(async () => stream);
        vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
        vi.stubGlobal('navigator', { mediaDevices: { getUserMedia } });
        const sendFile = vi.spyOn(app, 'sendFile').mockResolvedValue(undefined);

        await app.startRecording();
        expect(getUserMedia).toHaveBeenCalledWith({ audio: true });
        expect(app.recording).toBe(true);
        await app.stopRecording();

        expect(sendFile).toHaveBeenCalledTimes(1);
        const file = sendFile.mock.calls[0]?.[0] as File;
        expect(file.name).toMatch(/^voice-\d+\.webm$/);
        expect(file.type).toContain('audio/webm');
        expect(stopTrack).toHaveBeenCalled();
        expect(app.recording).toBe(false);
    });

    it('cancels recording without uploading', async () => {
        const { app } = makeApp();
        app.route = { kind: 'chat', id: 'user-1' };
        const stopTrack = vi.fn();
        const stream = {
            getTracks: () => [{ stop: stopTrack }],
        } as unknown as MediaStream;
        vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
        vi.stubGlobal('navigator', {
            mediaDevices: { getUserMedia: vi.fn(async () => stream) },
        });
        const sendFile = vi.spyOn(app, 'sendFile').mockResolvedValue(undefined);

        await app.startRecording();
        app.cancelRecording();

        expect(sendFile).not.toHaveBeenCalled();
        expect(stopTrack).toHaveBeenCalled();
        expect(app.recording).toBe(false);
    });

    it('clears all and scoped history through the bridge', async () => {
        const { app, client } = makeApp();
        app.route = { kind: 'settings', section: 'storage' };
        app.messages = [
            {
                id: 'message-1',
                conversation_id: 'conversation-1',
                user_id: 'user-1',
                sender_id: 'user-1',
                local_sender: false,
                seq: 1,
                sent_at: 1,
                received_at: 1,
                body: 'hello',
                attachment_hash: '',
                attachment_name: '',
                attachment_size: 0,
                attachment_mime: '',
                attachment_path: '',
                state: 'read',
                notify: true,
            },
        ];

        await app.clearHistory();
        expect(client.request).toHaveBeenCalledWith('clear_history', { user_id: '' });
        expect(app.messages).toEqual([]);
        expect(app.storageUsage).toEqual(USAGE);

        await app.clearHistory('user-2');
        expect(client.request).toHaveBeenCalledWith('clear_history', { user_id: 'user-2' });
    });

    it('loads real storage usage', async () => {
        const { app } = makeApp();
        await app.loadStorageUsage();
        expect(app.storageUsage).toEqual(USAGE);
    });

    it('applies saved discovery and appearance settings', async () => {
        const { app, client } = makeApp();
        app.settings = {
            'discovery.enabled': 'true',
            'appearance.color_scheme': 'system',
        };
        app.settingsDraft = {
            'discovery.enabled': 'false',
            'appearance.color_scheme': 'dark',
        };

        await app.saveSettings();

        expect(client.request).toHaveBeenCalledWith('settings_save', {
            key: 'discovery.enabled',
            value: 'false',
        });
        expect(client.request).toHaveBeenCalledWith('settings_save', {
            key: 'appearance.color_scheme',
            value: 'dark',
        });
        expect(document.documentElement.dataset.theme).toBe('dark');
    });

    it('applies system, light, and dark appearance', () => {
        const { app } = makeApp();
        app.settingsDraft = { 'appearance.color_scheme': 'dark' };
        app.applyAppearance();
        expect(document.documentElement.dataset.theme).toBe('dark');

        app.settingsDraft = { 'appearance.color_scheme': 'light' };
        app.applyAppearance();
        expect(document.documentElement.dataset.theme).toBe('light');

        app.settingsDraft = { 'appearance.color_scheme': 'system' };
        app.applyAppearance();
        expect(document.documentElement.dataset.theme).toBeUndefined();
    });

    it('keeps a remote request pending until the host accepts', () => {
        const { app } = makeApp();
        const session = {
            session_id: 'session-1',
            user_id: 'user-1',
            fingerprint: 'a'.repeat(64),
            role: 'host',
            state: 'requested',
            indicator: false,
            expires_at: 1,
        };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({ type: 'remote_session_requested', session });
        expect(app.remote?.state).toBe('requested');
        expect(app.remote?.indicator).toBe(false);

        handle({ type: 'remote_session_started', session: { ...session, state: 'active' } });
        expect(app.remote?.state).toBe('active');
        expect(app.remote?.indicator).toBe(true);
    });

    it('downloads and caches attachment URLs for playback', async () => {
        const { app, client } = makeApp();
        expect(await app.attachmentUrl('hash-1')).toBe('blob:attachment');
        expect(await app.attachmentUrl('hash-1')).toBe('blob:attachment');
        expect(client.downloadAttachment).toHaveBeenCalledTimes(1);
        expect(client.downloadAttachment).toHaveBeenCalledWith('hash-1');
    });
});

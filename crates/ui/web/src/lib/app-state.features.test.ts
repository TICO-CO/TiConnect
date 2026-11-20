import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AppState } from './app-state.svelte';
import { BridgeClient } from './bridge';
import type { StorageUsage, WatchParty, YoutubeStream } from './types';

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

describe('group watch party behavior', () => {
    const groupParty = {
        party_id: 'party-1',
        host_id: 'node-1',
        video_id: 'dQw4w9WgXcQ',
        role: 'host',
        state: 'ringing',
        participants: ['node-1'],
        invited: ['user-1', 'user-2'],
        position_ms: 0,
        playing: false,
        revision: 0,
        actor: '',
        updated_at: 1,
    };

    it('starts a group party with every selected user', async () => {
        const { app, client } = makeApp();
        app.bootstrap = {
            identity: {
                node_id: 'node-1',
                fingerprint: 'fingerprint',
                display_name: 'Host',
                avatar_hash: '',
                created_at: 1,
            },
            status: {
                transport_state: 'online',
                active_sessions: 2,
                outbox_depth: 0,
                user_count: 2,
            },
            settings: {},
            appearance: 'system',
            autostart: false,
            version: '0.0.6',
            online: true,
        };
        client.request.mockImplementation(async (method: string) => {
            if (method === 'start_watch_party') {
                return [groupParty, null];
            }
            return [null, null];
        });

        await app.startParty('dQw4w9WgXcQ', ['user-1', 'user-2']);

        expect(client.request).toHaveBeenCalledWith('start_watch_party', {
            user_ids: ['user-1', 'user-2'],
            video_id: 'dQw4w9WgXcQ',
        });
        expect(app.party?.participants).toEqual(['node-1']);
        expect(app.party?.invited).toEqual(['user-1', 'user-2']);
    });

    it('sends a video link to multiple people', async () => {
        const { app, client } = makeApp();
        await app.sendVideoToUsers(['user-1', 'user-2'], 'dQw4w9WgXcQ');

        expect(client.request).toHaveBeenCalledWith('send_message', {
            user_id: 'user-1',
            body: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
        });
        expect(client.request).toHaveBeenCalledWith('send_message', {
            user_id: 'user-2',
            body: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
        });
    });

    it('does not let a stale accept reply overwrite newer playback state', async () => {
        const { app, client } = makeApp();
        const invited: WatchParty = {
            ...groupParty,
            role: 'guest',
            participants: ['node-1'],
            invited: [],
        };
        app.party = invited;
        let resolveAccept: (value: [WatchParty | null, null]) => void = () => {};
        const accepting = new Promise<[WatchParty | null, null]>((resolve) => {
            resolveAccept = resolve;
        });
        client.request.mockImplementation(async (method: string) => {
            if (method === 'accept_watch_party') {
                return accepting;
            }
            return [null, null];
        });
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        const request = app.acceptParty();
        handle({
            type: 'watch_party_changed',
            party: {
                ...invited,
                participants: ['node-1', 'guest'],
                playing: true,
                position_ms: 42_000,
                revision: 2,
            },
        });
        resolveAccept([
            {
                ...invited,
                participants: ['node-1', 'guest'],
                playing: false,
                position_ms: 0,
                revision: 1,
            },
            null,
        ]);
        await request;

        expect(app.party?.playing).toBe(true);
        expect(app.party?.position_ms).toBe(42_000);
        expect(app.party?.revision).toBe(2);
    });

    it('prefetches a video stream once for concurrent callers', async () => {
        const { app, client } = makeApp();
        let resolveStream: (value: [YoutubeStream | null, null]) => void = () => {};
        const stream = new Promise<[YoutubeStream | null, null]>((resolve) => {
            resolveStream = resolve;
        });
        client.request.mockImplementation(async (method: string) => {
            if (method === 'youtube_stream') {
                return stream;
            }
            return [null, null];
        });

        app.prefetchYouTubeStream('video-1');
        expect(app.videoPreparing('video-1')).toBe(true);
        const playing = app.youtubeStream('video-1');
        expect(client.request).toHaveBeenCalledTimes(1);

        resolveStream([{ video_id: 'video-1', url: 'http://media/video-1' }, null]);

        expect(await playing).toBe('http://media/video-1');
        expect(app.videoPreparing('video-1')).toBe(false);

        app.prefetchYouTubeStream('video-1');
        expect(client.request).toHaveBeenCalledTimes(1);
    });

    it('prefetches the invited video before joining', async () => {
        const { app, client } = makeApp();
        client.request.mockImplementation(async (method: string) => {
            if (method === 'youtube_stream') {
                return [{ video_id: 'video-1', url: 'http://media/video-1' }, null];
            }
            return [null, null];
        });
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'watch_party_requested',
            party: { ...groupParty, role: 'guest', video_id: 'video-1' },
        });

        await vi.waitFor(() =>
            expect(client.request).toHaveBeenCalledWith('youtube_stream', {
                video_id: 'video-1',
            }),
        );
    });

    it('tracks transfer progress events', () => {
        const { app } = makeApp();
        (app as unknown as { handleEvent: (event: unknown) => void }).handleEvent({
            type: 'transfer_progress',
            hash: 'abc',
            bytes: 50,
            total: 100,
        });
        expect(app.transferFor('abc')).toEqual({
            bytes: 50,
            total: 100,
            done: false,
            failed: '',
        });
        (app as unknown as { handleEvent: (event: unknown) => void }).handleEvent({
            type: 'transfer_failed',
            hash: 'abc',
            reason: 'peer disconnected',
        });
        expect(app.transferFor('abc')?.failed).toBe('peer disconnected');
    });
});

describe('attachment download deduplication', () => {
    it('shares one download across concurrent attachment URL requests', async () => {
        const { app, client } = makeApp();
        let resolveDownload: (value: [string | null, null]) => void = () => {};
        const pending = new Promise<[string | null, null]>((resolve) => {
            resolveDownload = resolve;
        });
        client.downloadAttachment.mockReturnValue(pending);

        const first = app.attachmentUrl('hash-1');
        const second = app.attachmentUrl('hash-1');
        expect(client.downloadAttachment).toHaveBeenCalledTimes(1);

        resolveDownload(['blob:attachment', null]);
        expect(await first).toBe('blob:attachment');
        expect(await second).toBe('blob:attachment');
        expect(await app.attachmentUrl('hash-1')).toBe('blob:attachment');
        expect(client.downloadAttachment).toHaveBeenCalledTimes(1);
    });
});

describe('watch party navigation', () => {
    it('opens the watch party page with preselected users', () => {
        const { app } = makeApp();
        app.openWatchParty(['user-1', 'user-1']);

        expect(app.route).toEqual({ kind: 'watch-party' });
        expect(app.takeWatchPartySelection()).toEqual(['user-1']);
        expect(app.takeWatchPartySelection()).toEqual([]);
    });
});

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AppState } from './app-state.svelte';
import { BridgeClient } from './bridge';
import { type BridgeFailure, BridgeFailures } from './bridge-failures';
import { err, ok, type Result } from './result';
import type { CallLog, StorageUsage, WatchParty, YoutubeStream } from './types';

const USAGE: StorageUsage = {
    message_count: 4,
    attachment_count: 2,
    attachment_bytes: 2048,
    database_bytes: 4096,
};

const CALL_LOG: CallLog = {
    id: 'call-1',
    user_id: 'user-1',
    direction: 'outgoing',
    started_at: 1_000,
    connected_at: 2_000,
    ended_at: 62_000,
    duration_secs: 60,
    reason: 'hangup',
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
                return ok([]);
            case 'list_conversation_summaries':
                return ok([]);
            case 'watch_party':
                return ok(null);
            case 'storage_usage':
                return ok(USAGE);
            case 'list_call_logs':
                return ok([CALL_LOG]);
            case 'list_audio_devices':
                return ok([{ id: 'Mic', name: 'Mic', kind: 'input' }]);
            case 'clear_history':
                return ok(1);
            case 'list_settings':
                return ok({ 'appearance.color_scheme': 'dark' });
            case 'autostart_get':
                return ok(false);
            case 'settings_save':
                return ok(null);
            default:
                return ok(null);
        }
    });
    const uploadAttachment = vi.fn(async () => ok('message-1'));
    const downloadAttachment = vi.fn(async () => ok('blob:attachment'));
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

    afterEach(() => {
        vi.useRealTimers();
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

    it('keeps forming waveform bars while the input holds steady', async () => {
        vi.useFakeTimers();
        const { app } = makeApp();
        app.route = { kind: 'chat', id: 'user-1' };
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
        const gainNode = { connect, gain: { value: 0 } };
        class FakeAudioContext {
            state: AudioContextState = 'running';
            resume = vi.fn(async () => undefined);
            close = vi.fn(async () => undefined);
            createMediaStreamSource = vi.fn(() => ({ connect }));
            createGain = vi.fn(() => gainNode);
            createMediaStreamDestination = vi.fn(() => ({ connect, stream }));
            createAnalyser = vi.fn(() => analyser);
        }
        vi.stubGlobal('AudioContext', FakeAudioContext);
        app.settings = { 'audio.input_volume': '50' };
        const sendFile = vi.spyOn(app, 'sendFile').mockResolvedValue(undefined);

        await app.startRecording();
        expect(app.recording).toBe(true);
        expect(gainNode.gain.value).toBe(0.5);
        await vi.advanceTimersByTimeAsync(350);

        expect(app.recordingLevel).toBeGreaterThan(0.5);
        expect(app.recordingWaveform.length).toBeGreaterThanOrEqual(3);
        await app.stopRecording();

        expect(sendFile).toHaveBeenCalledTimes(1);
        expect(app.recording).toBe(false);
        expect(app.recordingWaveform).toEqual([]);
        vi.useRealTimers();
    });

    it('reports an unavailable microphone level meter', async () => {
        vi.useFakeTimers();
        const { app } = makeApp();
        app.route = { kind: 'chat', id: 'user-1' };
        const stream = {
            getTracks: () => [{ stop: vi.fn() }],
        } as unknown as MediaStream;
        vi.stubGlobal('MediaRecorder', FakeMediaRecorder);
        vi.stubGlobal('navigator', {
            mediaDevices: { getUserMedia: vi.fn(async () => stream) },
        });
        class SuspendedAudioContext {
            state: AudioContextState = 'suspended';
            resume = vi.fn(async () => undefined);
            close = vi.fn(async () => undefined);
        }
        vi.stubGlobal('AudioContext', SuspendedAudioContext);

        await app.startRecording();
        await vi.advanceTimersByTimeAsync(10);

        expect(app.recording).toBe(true);
        expect(app.notice).toBe('Microphone level is unavailable.');
        expect(app.recordingWaveform).toEqual([]);
        app.cancelRecording();
        vi.useRealTimers();
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

    it('loads persisted call logs and audio devices', async () => {
        const { app } = makeApp();
        await app.loadCallLogs();
        await app.loadAudioDevices();
        expect(app.callLogs).toEqual([CALL_LOG]);
        expect(app.audioDevices).toEqual([{ id: 'Mic', name: 'Mic', kind: 'input' }]);
    });

    it('stores per-user volume and syncs the active call', async () => {
        const { app, client } = makeApp();
        app.call = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'outgoing',
            state: 'in-call',
            started_at: 1,
            connected_at: 2,
            input_muted: false,
            output_muted: false,
        };

        await app.setUserVolume('user-1', 86);

        expect(app.userVolume('user-1')).toBe(86);
        expect(client.request).toHaveBeenCalledWith('set_call_volume', {
            call_id: 'call-1',
            volume: 86,
        });
        expect(app.userVolume('user-2')).toBe(100);
    });

    it('multiplies user volume with global output volume', async () => {
        const { app } = makeApp();
        app.settings = { 'audio.output_volume': '50' };
        await app.setUserVolume('user-1', 80);

        expect(app.messageOutputVolume('user-1')).toBeCloseTo(0.4);
        expect(app.messageOutputVolume('user-2')).toBeCloseTo(0.5);
    });

    it('toggles call mute through the bridge', async () => {
        const { app, client } = makeApp();
        app.call = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'outgoing',
            state: 'in-call',
            started_at: 1,
            connected_at: 2,
            input_muted: false,
            output_muted: false,
        };

        await app.toggleInputMute();

        expect(client.request).toHaveBeenCalledWith('set_call_muted', {
            call_id: 'call-1',
            input_muted: true,
            output_muted: false,
        });
        expect(app.call?.input_muted).toBe(true);
    });

    it('accepts a ringing call only once while the request is pending', async () => {
        const { app, client } = makeApp();
        const ringingCall = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'incoming',
            state: 'ringing',
            started_at: 1,
            connected_at: 0,
            input_muted: false,
            output_muted: false,
        };
        app.call = ringingCall;
        let releaseAccept: (value: Result<unknown, BridgeFailure>) => void = () => undefined;
        const pendingAccept = new Promise<Result<unknown, BridgeFailure>>((resolve) => {
            releaseAccept = resolve;
        });
        client.request.mockImplementation(async (method: string) => {
            if (method === 'accept_call') {
                return pendingAccept;
            }
            if (method === 'call_state') {
                return ok({ ...ringingCall, state: 'in-call' });
            }
            return ok(null);
        });

        const first = app.acceptCall();
        const second = app.acceptCall();
        expect(
            client.request.mock.calls.filter(([method]) => method === 'accept_call'),
        ).toHaveLength(1);

        releaseAccept(ok(null));
        await first;
        await second;
        expect(app.call?.state).toBe('in-call');
    });

    it('maps call metrics to the waveform level', async () => {
        const { app, client } = makeApp();
        const activeCall = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'incoming',
            state: 'in-call',
            started_at: 1,
            connected_at: 2,
            input_muted: false,
            output_muted: false,
        };
        app.call = activeCall;
        client.request.mockImplementation(async (method: string) => {
            if (method === 'call_metrics') {
                return ok({ output_level: 640 });
            }
            return ok(null);
        });
        const poll = (app as unknown as { pollCallLevel: () => Promise<void> }).pollCallLevel.bind(
            app,
        );

        await poll();
        expect(app.callLevel).toBeCloseTo(0.64);

        app.call = { ...activeCall, output_muted: true };
        await poll();
        expect(app.callLevel).toBe(0);
    });

    it('clears the waveform level when metrics fail or the call is gone', async () => {
        const { app, client } = makeApp();
        app.call = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'incoming',
            state: 'in-call',
            started_at: 1,
            connected_at: 2,
            input_muted: false,
            output_muted: false,
        };
        client.request.mockImplementation(async (method: string) => {
            if (method === 'call_metrics') {
                return err(BridgeFailures.notConnected());
            }
            return ok(null);
        });
        const poll = (app as unknown as { pollCallLevel: () => Promise<void> }).pollCallLevel.bind(
            app,
        );

        app.callLevel = 0.8;
        await poll();
        expect(app.callLevel).toBe(0);

        app.call = null;
        app.callLevel = 0.5;
        await poll();
        expect(app.callLevel).toBe(0);
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

    it('saves live volume changes after the debounce', async () => {
        vi.useFakeTimers();
        const { app, client } = makeApp();
        app.settings = { 'audio.output_volume': '100' };
        app.settingsDraft = { 'audio.output_volume': '100' };

        app.setDraftSettingLive('audio.output_volume', '70');
        app.setDraftSettingLive('audio.output_volume', '35');

        expect(client.request).not.toHaveBeenCalledWith('settings_save', {
            key: 'audio.output_volume',
            value: '35',
        });

        await vi.advanceTimersByTimeAsync(100);

        expect(client.request).toHaveBeenCalledWith('settings_save', {
            key: 'audio.output_volume',
            value: '35',
        });
        expect(app.settings['audio.output_volume']).toBe('35');
        expect(app.settingsSave).toBe('saved');
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
            input_allowed: true,
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

    it('refuses to send while offline or over the body limit', async () => {
        const { app, client } = makeApp();
        app.route = { kind: 'chat', id: 'user-1' };
        app.connection = 'offline';
        app.draft = 'hello';
        await app.sendMessage();
        expect(client.request).not.toHaveBeenCalledWith('send_message', expect.anything());

        app.connection = 'online';
        app.draft = 'a'.repeat(32_769);
        await app.sendMessage();
        expect(client.request).not.toHaveBeenCalledWith('send_message', expect.anything());
        expect(app.draft).toHaveLength(32_769);
    });

    it('applies edited and deleted message events', () => {
        const { app } = makeApp();
        app.messages = [
            {
                id: 'message-1',
                conversation_id: 'conversation-1',
                user_id: 'user-1',
                sender_id: 'user-1',
                local_sender: true,
                seq: 1,
                sent_at: 1_000,
                received_at: 1_000,
                body: 'original',
                reply_to: '',
                attachment_hash: '',
                attachment_name: '',
                attachment_size: 0,
                attachment_mime: '',
                attachment_path: '',
                state: 'sent',
                edited_at: 0,
                deleted_at: 0,
                notify: true,
            },
        ];
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'message_edited',
            message_id: 'message-1',
            user_id: 'user-1',
            body: 'updated',
            edited_at: 2_000,
        });
        expect(app.messages[0]?.body).toBe('updated');
        expect(app.messages[0]?.edited_at).toBe(2_000);

        handle({
            type: 'message_deleted',
            message_id: 'message-1',
            user_id: 'user-1',
        });
        expect(app.messages[0]?.deleted_at).toBeGreaterThan(0);
    });

    it('sends a reply with the quoted message id and clears the reply target', async () => {
        const { app, client } = makeApp();
        client.request.mockImplementation(async (method: string) => {
            if (method === 'send_message') {
                return ok('message-9');
            }
            return ok(null);
        });
        app.route = { kind: 'chat', id: 'user-1' };
        app.connection = 'online';
        app.draft = 'answer';
        app.replyToId = 'message-1';
        await app.sendMessage();

        expect(client.request).toHaveBeenCalledWith('send_message', {
            user_id: 'user-1',
            body: 'answer',
            reply_to: 'message-1',
        });
        expect(app.replyToId).toBe('');
        expect(app.draft).toBe('');
    });

    it('edits and deletes messages locally in the demo', async () => {
        const { app } = makeApp();
        app.demo = true;
        app.messages = [
            {
                id: 'message-1',
                conversation_id: 'conversation-1',
                user_id: 'user-1',
                sender_id: 'user-1',
                local_sender: true,
                seq: 1,
                sent_at: 1_000,
                received_at: 1_000,
                body: 'original',
                reply_to: '',
                attachment_hash: '',
                attachment_name: '',
                attachment_size: 0,
                attachment_mime: '',
                attachment_path: '',
                state: 'sent',
                edited_at: 0,
                deleted_at: 0,
                notify: true,
            },
        ];

        expect(await app.editMessage('message-1', 'updated')).toBe(true);
        expect(app.messages[0]?.body).toBe('updated');
        expect(app.messages[0]?.edited_at).toBeGreaterThan(0);

        expect(await app.deleteMessage('message-1')).toBe(true);
        expect(app.messages[0]?.deleted_at).toBeGreaterThan(0);
        expect(await app.editMessage('message-1', 'again')).toBe(false);
    });

    it('applies remote input policy changes from the daemon', () => {
        const { app } = makeApp();
        app.remote = {
            session_id: 'session-1',
            user_id: 'user-1',
            fingerprint: 'a'.repeat(64),
            role: 'viewer',
            state: 'active',
            indicator: true,
            input_allowed: true,
            expires_at: 1,
        };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'remote_input_changed',
            session_id: 'session-1',
            allowed: false,
        });

        expect(app.remote?.input_allowed).toBe(false);
    });

    it('keeps the reason after a remote session ends', () => {
        const { app } = makeApp();
        app.remote = {
            session_id: 'session-1',
            user_id: 'user-1',
            fingerprint: 'a'.repeat(64),
            role: 'viewer',
            state: 'active',
            indicator: true,
            input_allowed: true,
            expires_at: 1,
        };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'remote_session_ended',
            session_id: 'session-1',
            reason: 'expired',
        });

        expect(app.remote).toBeNull();
        expect(app.remoteOutcome).toEqual({
            userId: 'user-1',
            role: 'viewer',
            reason: 'expired',
            endedAt: expect.any(Number),
        });
    });

    it('downloads and caches attachment URLs for playback', async () => {
        const { app, client } = makeApp();
        expect(await app.attachmentUrl('hash-1')).toBe('blob:attachment');
        expect(await app.attachmentUrl('hash-1')).toBe('blob:attachment');
        expect(client.downloadAttachment).toHaveBeenCalledTimes(1);
        expect(client.downloadAttachment).toHaveBeenCalledWith('hash-1');
    });

    it('auto-dismisses message toasts after the overlay timeout', () => {
        vi.useFakeTimers();
        const { app } = makeApp();
        app.demo = true;
        app.settings = { 'overlay.timeout_secs': '2' };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'message_received',
            message: {
                id: 'message-1',
                conversation_id: 'conversation-1',
                user_id: 'user-1',
                sender_id: 'user-1',
                local_sender: false,
                seq: 1,
                sent_at: 1_000,
                received_at: 1_000,
                body: 'hello',
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
            },
        });

        expect(app.toasts).toHaveLength(1);
        vi.advanceTimersByTime(1_999);
        expect(app.toasts).toHaveLength(1);
        vi.advanceTimersByTime(1);
        expect(app.toasts).toHaveLength(0);
    });

    it('applies the daemon in-call state string', () => {
        const { app } = makeApp();
        app.call = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'incoming',
            state: 'ringing',
            started_at: 1_000,
            connected_at: 0,
            input_muted: false,
            output_muted: false,
        };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({ type: 'call_state_changed', call_id: 'call-1', state: 'in-call' });

        expect(app.call?.state).toBe('in-call');
    });

    it('auto-dismisses call ended toasts after the overlay timeout', () => {
        vi.useFakeTimers();
        const { app } = makeApp();
        app.settings = { 'overlay.timeout_secs': '1' };
        app.call = {
            id: 'call-1',
            user_id: 'user-1',
            direction: 'outgoing',
            state: 'in-call',
            started_at: 1_000,
            connected_at: 2_000,
            input_muted: false,
            output_muted: false,
        };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({ type: 'call_ended', call_id: 'call-1', reason: 'hangup' });

        expect(app.toasts).toHaveLength(1);
        vi.advanceTimersByTime(1_000);
        expect(app.toasts).toHaveLength(0);
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
            settings: {},
            appearance: 'system',
            autostart: false,
            version: '0.1.1',
            dev_build: false,
            online: true,
        };
        client.request.mockImplementation(async (method: string) => {
            if (method === 'start_watch_party') {
                return ok(groupParty);
            }
            return ok(null);
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
        let resolveAccept: (value: Result<WatchParty, BridgeFailure>) => void = () => {};
        const accepting = new Promise<Result<WatchParty, BridgeFailure>>((resolve) => {
            resolveAccept = resolve;
        });
        client.request.mockImplementation(async (method: string) => {
            if (method === 'accept_watch_party') {
                return accepting;
            }
            return ok(null);
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
        resolveAccept(
            ok({
                ...invited,
                participants: ['node-1', 'guest'],
                playing: false,
                position_ms: 0,
                revision: 1,
            }),
        );
        await request;

        expect(app.party?.playing).toBe(true);
        expect(app.party?.position_ms).toBe(42_000);
        expect(app.party?.revision).toBe(2);
    });

    it('prefetches a video stream once for concurrent callers', async () => {
        const { app, client } = makeApp();
        let resolveStream: (value: Result<YoutubeStream, BridgeFailure>) => void = () => {};
        const stream = new Promise<Result<YoutubeStream, BridgeFailure>>((resolve) => {
            resolveStream = resolve;
        });
        client.request.mockImplementation(async (method: string) => {
            if (method === 'youtube_stream') {
                return stream;
            }
            return ok(null);
        });

        app.prefetchYouTubeStream('video-1');
        expect(app.videoPreparing('video-1')).toBe(true);
        const playing = app.youtubeStream('video-1');
        expect(client.request).toHaveBeenCalledTimes(1);

        resolveStream(ok({ video_id: 'video-1', url: 'http://media/video-1' }));

        expect(await playing).toBe('http://media/video-1');
        expect(app.videoPreparing('video-1')).toBe(false);

        app.prefetchYouTubeStream('video-1');
        expect(client.request).toHaveBeenCalledTimes(1);
    });

    it('keeps the reason after a watch party ends', () => {
        const { app } = makeApp();
        app.party = { ...groupParty, state: 'active' };
        const handle = (app as unknown as { handleEvent(event: unknown): void }).handleEvent.bind(
            app,
        );

        handle({
            type: 'watch_party_ended',
            party_id: 'party-1',
            reason: 'host disconnected',
        });

        expect(app.party).toBeNull();
        expect(app.partyOutcome).toEqual({
            partyId: 'party-1',
            hostId: 'node-1',
            reason: 'host disconnected',
            endedAt: expect.any(Number),
        });
    });

    it('prefetches the invited video before joining', async () => {
        const { app, client } = makeApp();
        client.request.mockImplementation(async (method: string) => {
            if (method === 'youtube_stream') {
                return ok({ video_id: 'video-1', url: 'http://media/video-1' });
            }
            return ok(null);
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
        let resolveDownload: (value: Result<string, BridgeFailure>) => void = () => {};
        const pending = new Promise<Result<string, BridgeFailure>>((resolve) => {
            resolveDownload = resolve;
        });
        client.downloadAttachment.mockReturnValue(pending);

        const first = app.attachmentUrl('hash-1');
        const second = app.attachmentUrl('hash-1');
        expect(client.downloadAttachment).toHaveBeenCalledTimes(1);

        resolveDownload(ok('blob:attachment'));
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

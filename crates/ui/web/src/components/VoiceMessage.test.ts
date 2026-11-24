import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { AppState, Message } from '../lib';
import VoiceMessage from './VoiceMessage.svelte';

const MESSAGE: Message = {
    id: 'message-audio',
    conversation_id: 'conversation-1',
    user_id: 'user-1',
    sender_id: 'user-1',
    local_sender: false,
    seq: 1,
    sent_at: 1_700_000_000,
    received_at: 1_700_000_000,
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

function makeApp(overrides: Record<string, unknown> = {}): AppState {
    return {
        transferFor: () => null,
        attachmentUrl: async () => 'blob:voice',
        ...overrides,
    } as unknown as AppState;
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

function voiceBars(container: HTMLElement): HTMLElement[] {
    return Array.from(container.querySelectorAll<HTMLElement>('span[style*="width: 3px"]'));
}

describe('VoiceMessage', () => {
    it('renders only the player once the waveform loads', async () => {
        stubDecodableAudio();
        const { container } = render(VoiceMessage, { app: makeApp(), message: MESSAGE });

        await waitFor(() => expect(screen.getByText('00:06')).toBeTruthy());
        expect(screen.getByRole('button', { name: 'Play voice message' })).toBeTruthy();
        expect(voiceBars(container)).toHaveLength(30);
        expect(screen.queryByText('voice.webm')).toBeNull();
        expect(container.querySelector('audio')?.getAttribute('src')).toBe('blob:voice');
    });

    it('plays, pauses, and seeks with the cursor', async () => {
        stubDecodableAudio();
        const { container } = render(VoiceMessage, { app: makeApp(), message: MESSAGE });
        await waitFor(() => expect(screen.getByText('00:06')).toBeTruthy());

        const audio = container.querySelector('audio');
        expect(audio).not.toBeNull();
        await fireEvent.click(screen.getByRole('button', { name: 'Play voice message' }));
        expect(window.HTMLMediaElement.prototype.play).toHaveBeenCalled();
        audio?.dispatchEvent(new Event('play'));
        await waitFor(() =>
            expect(screen.getByRole('button', { name: 'Pause voice message' })).toBeTruthy(),
        );

        const slider = screen.getByRole('slider', { name: 'Seek voice message' });
        await fireEvent.input(slider, { target: { value: '3' } });
        expect(audio?.currentTime).toBe(3);

        audio?.dispatchEvent(new Event('pause'));
        await waitFor(() =>
            expect(screen.getByRole('button', { name: 'Play voice message' })).toBeTruthy(),
        );
    });

    it('retries when the voice file cannot load', async () => {
        const attachmentUrl = vi.fn(async () => '');
        render(VoiceMessage, { app: makeApp({ attachmentUrl }), message: MESSAGE });

        await waitFor(() => expect(screen.getByText('Voice message unavailable')).toBeTruthy());
        await fireEvent.click(screen.getByRole('button', { name: 'Retry voice message' }));
        expect(attachmentUrl).toHaveBeenCalledTimes(2);
    });
});

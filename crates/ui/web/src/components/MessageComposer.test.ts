import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { Bootstrap, User } from '../lib';
import { AppState } from '../lib';
import MessageComposer from './MessageComposer.svelte';

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

function makeApp(): AppState {
    const app = new AppState();
    const bootstrap = {
        identity: {
            node_id: 'node-1',
            fingerprint: USER.fingerprint,
            display_name: USER.display_name,
            avatar_hash: '',
            created_at: CREATED_AT,
        },
        appearance: 'system',
    } as unknown as Bootstrap;
    app.bootstrap = bootstrap;
    app.connection = 'online';
    app.users = [USER];
    app.route = { kind: 'chat', id: USER.id };
    return app;
}

function stubResizeObserver(): void {
    class FakeResizeObserver {
        observe(): void {}
        disconnect(): void {}
        unobserve(): void {}
    }
    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
}

describe('MessageComposer', () => {
    it('shows the mic action for an empty draft', () => {
        stubResizeObserver();
        const app = makeApp();
        app.draft = '';
        render(MessageComposer, { app });

        expect(screen.getByPlaceholderText('Write a message')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Record voice message' })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Attach file' })).toBeTruthy();
        expect(screen.queryByRole('button', { name: 'Send message' })).toBeNull();
    });

    it('shows the send action once text is typed', async () => {
        stubResizeObserver();
        const app = makeApp();
        app.draft = '';
        render(MessageComposer, { app });
        const field = screen.getByPlaceholderText('Write a message');
        await fireEvent.input(field, { target: { value: 'hello' } });

        expect(screen.getByRole('button', { name: 'Send message' })).toBeTruthy();
        expect(screen.queryByRole('button', { name: 'Record voice message' })).toBeNull();
    });

    it('sends the draft on submit', async () => {
        stubResizeObserver();
        const app = makeApp();
        app.draft = 'hello';
        const send = vi.spyOn(app, 'sendMessage').mockResolvedValue(undefined);
        render(MessageComposer, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Send message' }));
        expect(send).toHaveBeenCalled();
    });

    it('replaces the input with live recording bars', () => {
        stubResizeObserver();
        const app = makeApp();
        app.recording = true;
        app.recordingSeconds = 65;
        app.recordingWaveform = [0.2, 0.8, 0.4];
        const { container } = render(MessageComposer, { app });

        expect(screen.getByText('00:01:05')).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Cancel voice message' })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Stop and send voice message' })).toBeTruthy();
        expect(screen.queryByPlaceholderText('Write a message')).toBeNull();
        expect(container.querySelectorAll('span[class*="w-[3px]"]').length).toBe(3);
    });

    it('disables the mic while offline', () => {
        stubResizeObserver();
        const app = makeApp();
        app.connection = 'offline';
        render(MessageComposer, { app });

        expect(screen.getByRole('button', { name: 'Record voice message' })).toHaveProperty(
            'disabled',
            true,
        );
        expect(
            screen.getByPlaceholderText('Daemon offline — messages cannot be sent.'),
        ).toBeTruthy();
    });

    it('stops and cancels recording from the composer', async () => {
        stubResizeObserver();
        const app = makeApp();
        app.recording = true;
        const stop = vi.spyOn(app, 'stopRecording').mockResolvedValue(undefined);
        const cancel = vi.spyOn(app, 'cancelRecording').mockImplementation(() => {});
        render(MessageComposer, { app });

        await fireEvent.click(screen.getByRole('button', { name: 'Stop and send voice message' }));
        expect(stop).toHaveBeenCalled();
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel voice message' }));
        expect(cancel).toHaveBeenCalled();
        await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    });

    it('caps the input at three lines and scrolls beyond', async () => {
        stubResizeObserver();
        const app = makeApp();
        app.draft = '';
        render(MessageComposer, { app });
        const field = screen.getByPlaceholderText('Write a message') as HTMLTextAreaElement;

        expect(field.className).toContain('max-h-[4.35em]');
        expect(field.className).toContain('overflow-y-auto');

        Object.defineProperty(field, 'scrollHeight', { value: 200, configurable: true });
        await fireEvent.input(field, { target: { value: 'line one\nline two' } });

        expect(field.style.height).toBe('200px');
    });
});

import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { AppState, WatchParty } from '../lib';
import YoutubePlayer from './YoutubePlayer.svelte';

const HOST_ID = 'node-1';
const GUEST_ID = 'node-2';

const PARTY: WatchParty = {
    party_id: 'party-1',
    host_id: 'node-1',
    video_id: 'ZjcyRr-f5Pc',
    role: 'host',
    state: 'active',
    participants: ['node-1', 'node-2'],
    invited: [],
    position_ms: 0,
    playing: false,
    revision: 1,
    actor: 'node-1',
    updated_at: 1,
};

function makeApp(stream: string | null): AppState {
    return {
        youtubeStream: vi.fn().mockResolvedValue(stream),
        audioOutputVolume: () => 100,
    } as unknown as AppState;
}

interface FakeMedia {
    paused: boolean;
    currentTime: number;
}

function installFakeMedia(video: HTMLVideoElement): FakeMedia {
    const media: FakeMedia = { paused: true, currentTime: 0 };
    Object.defineProperty(video, 'paused', {
        configurable: true,
        get: () => media.paused,
    });
    Object.defineProperty(video, 'currentTime', {
        configurable: true,
        get: () => media.currentTime,
        set: (value: number) => {
            media.currentTime = value;
        },
    });
    Object.defineProperty(video, 'duration', {
        configurable: true,
        get: () => 300,
    });
    Object.defineProperty(video, 'readyState', {
        configurable: true,
        get: () => 4,
    });
    video.play = () => {
        media.paused = false;
        void fireEvent.play(video);
        void fireEvent.playing(video);
        return Promise.resolve();
    };
    video.pause = () => {
        media.paused = true;
        void fireEvent.pause(video);
    };
    return media;
}

describe('youtube player', () => {
    it('loads a native video only after pressing play', async () => {
        const app = makeApp('https://example.com/video.mp4');
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        expect(screen.getByRole('button', { name: 'Play video' })).toBeTruthy();
        expect(document.querySelector('video')).toBeNull();

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));

        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        expect(document.querySelector('video')?.getAttribute('src')).toBe(
            'https://example.com/video.mp4',
        );
    });

    it('does not mount the video before the stream url resolves', async () => {
        let resolveStream: (value: string | null) => void = () => {};
        const stream = new Promise<string | null>((resolve) => {
            resolveStream = resolve;
        });
        const app = {
            youtubeStream: vi.fn(() => stream),
            audioOutputVolume: () => 100,
        } as unknown as AppState;
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));

        await waitFor(() => expect(app.youtubeStream).toHaveBeenCalledOnce());
        expect(document.querySelector('video')).toBeNull();

        resolveStream('https://example.com/video.mp4');

        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        expect(document.querySelector('video')?.getAttribute('src')).toBe(
            'https://example.com/video.mp4',
        );
        expect(screen.queryByText('Video unavailable.')).toBeNull();
    });

    it('clears a stale media error after a valid source loads', async () => {
        const app = makeApp('https://example.com/video.mp4');
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));

        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        const video = document.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }

        await fireEvent.error(video);
        expect(screen.getByText('Video unavailable.')).toBeTruthy();

        await fireEvent.loadedMetadata(video);
        expect(screen.queryByText('Video unavailable.')).toBeNull();
    });

    it('keeps the video visible while buffering after playback starts', async () => {
        const party: WatchParty = { ...PARTY, playing: true };
        const app = makeApp('https://example.com/video.mp4');
        render(YoutubePlayer, { app, party, selfId: 'node-1', onPublish: () => {} });

        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        const video = document.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }

        await fireEvent.loadedMetadata(video);
        await fireEvent.waiting(video);

        expect(screen.queryByRole('button', { name: 'Play video' })).toBeNull();
        expect(screen.queryByText('Starting video…')).toBeNull();
        expect(screen.getByText('Buffering…')).toBeTruthy();

        await fireEvent.timeUpdate(video);
        expect(screen.queryByText('Buffering…')).toBeNull();
    });

    it('controls volume and mute independently', async () => {
        const app = makeApp('https://example.com/video.mp4');
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));
        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        const video = document.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }

        await fireEvent.loadedMetadata(video);

        const volume = screen.getByRole('slider', { name: 'Volume' });
        await fireEvent.input(volume, { target: { value: '0.4' } });
        expect(video.volume).toBeCloseTo(0.4);

        await fireEvent.click(screen.getByRole('button', { name: 'Mute' }));
        expect(video.muted).toBe(true);

        await fireEvent.click(screen.getByRole('button', { name: 'Unmute' }));
        expect(video.muted).toBe(false);
    });

    it('scales local volume by global output volume', async () => {
        const app = {
            youtubeStream: vi.fn().mockResolvedValue('https://example.com/video.mp4'),
            audioOutputVolume: () => 50,
        } as unknown as AppState;
        const { container } = render(YoutubePlayer, {
            app,
            party: PARTY,
            selfId: 'node-1',
            onPublish: () => {},
        });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));
        await waitFor(() => expect(container.querySelector('video')).not.toBeNull());
        const video = container.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }
        installFakeMedia(video);
        await fireEvent.loadedMetadata(video);

        const volume = screen.getByRole('slider', { name: 'Volume' });
        await fireEvent.input(volume, { target: { value: '0.8' } });
        expect(video.volume).toBeCloseTo(0.4);
    });

    it('disables seeking until metadata is available', async () => {
        const app = makeApp('https://example.com/video.mp4');
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));
        await waitFor(() => expect(document.querySelector('video')).not.toBeNull());
        const video = document.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }

        const seek = screen.getByRole('slider', { name: 'Seek video' });
        expect((seek as HTMLInputElement).disabled).toBe(true);

        await fireEvent.loadedMetadata(video);
        expect((seek as HTMLInputElement).disabled).toBe(false);
    });

    it('syncs playback between host and guest watchers', async () => {
        let current: WatchParty = {
            ...PARTY,
            host_id: HOST_ID,
            participants: [HOST_ID, GUEST_ID],
            actor: HOST_ID,
        };
        const hostApp = makeApp('https://example.com/video.mp4');
        const guestApp = makeApp('https://example.com/video.mp4');
        let hostView: ReturnType<typeof render> | null = null;
        let guestView: ReturnType<typeof render> | null = null;

        function broadcast(next: WatchParty): void {
            current = next;
            void hostView?.rerender({
                app: hostApp,
                party: next,
                selfId: HOST_ID,
                onPublish: hostPublish,
            });
            void guestView?.rerender({
                app: guestApp,
                party: next,
                selfId: GUEST_ID,
                onPublish: guestPublish,
            });
        }

        function hostPublish(positionMs: number, playing: boolean): void {
            broadcast({
                ...current,
                position_ms: positionMs,
                playing,
                revision: current.revision + 1,
                actor: HOST_ID,
            });
        }

        function guestPublish(positionMs: number, playing: boolean): void {
            broadcast({
                ...current,
                position_ms: positionMs,
                playing,
                revision: current.revision + 1,
                actor: GUEST_ID,
            });
        }

        hostView = render(YoutubePlayer, {
            app: hostApp,
            party: current,
            selfId: HOST_ID,
            onPublish: hostPublish,
        });
        guestView = render(YoutubePlayer, {
            app: guestApp,
            party: current,
            selfId: GUEST_ID,
            onPublish: guestPublish,
        });

        await fireEvent.click(
            within(hostView.container).getByRole('button', { name: 'Play video' }),
        );
        await waitFor(() => expect(hostView?.container.querySelector('video')).not.toBeNull());
        const hostVideo = hostView.container.querySelector<HTMLVideoElement>('video');
        expect(hostVideo).not.toBeNull();
        if (hostVideo === null) {
            return;
        }
        const hostMedia = installFakeMedia(hostVideo);
        await fireEvent.loadedMetadata(hostVideo);

        await waitFor(() => expect(guestView?.container.querySelector('video')).not.toBeNull());
        const guestVideo = guestView.container.querySelector<HTMLVideoElement>('video');
        expect(guestVideo).not.toBeNull();
        if (guestVideo === null) {
            return;
        }
        const guestMedia = installFakeMedia(guestVideo);
        await fireEvent.loadedMetadata(guestVideo);

        await waitFor(() => expect(guestMedia.paused).toBe(false));
        expect(hostMedia.paused).toBe(false);

        const hostSeek = within(hostView.container).getByRole('slider', {
            name: 'Seek video',
        });
        await fireEvent.input(hostSeek, { target: { value: '42' } });

        await waitFor(() => expect(guestMedia.currentTime).toBeCloseTo(42));
        expect(guestMedia.paused).toBe(false);

        await fireEvent.click(within(hostView.container).getByRole('button', { name: 'Pause' }));
        await waitFor(() => expect(guestMedia.paused).toBe(true));

        const guestSeek = within(guestView.container).getByRole('slider', {
            name: 'Seek video',
        });
        await fireEvent.input(guestSeek, { target: { value: '60' } });
        await waitFor(() => expect(hostMedia.currentTime).toBeCloseTo(60));

        await fireEvent.click(within(guestView.container).getByRole('button', { name: 'Play' }));
        await waitFor(() => expect(hostMedia.paused).toBe(false));
    });

    it('publishes the host position while playing', async () => {
        const interval = vi.spyOn(window, 'setInterval');
        const onPublish = vi.fn();
        const app = makeApp('https://example.com/video.mp4');
        const party: WatchParty = { ...PARTY, playing: true };
        const view = render(YoutubePlayer, {
            app,
            party,
            selfId: HOST_ID,
            onPublish,
        });

        await waitFor(() => expect(view.container.querySelector('video')).not.toBeNull());
        const video = view.container.querySelector<HTMLVideoElement>('video');
        expect(video).not.toBeNull();
        if (video === null) {
            return;
        }
        installFakeMedia(video);
        await fireEvent.loadedMetadata(video);

        await waitFor(() =>
            expect(interval.mock.calls.some(([, delay]) => delay === 2_000)).toBe(true),
        );
        const heartbeat = interval.mock.calls.find(([, delay]) => delay === 2_000)?.[0];
        if (typeof heartbeat === 'function') {
            heartbeat();
        }
        expect(onPublish).toHaveBeenCalledWith(0, true);
    });

    it('shows a retry when the stream cannot be resolved', async () => {
        const app = makeApp(null);
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));

        await waitFor(() => expect(screen.getByText('Video unavailable.')).toBeTruthy());
        expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
    });
});

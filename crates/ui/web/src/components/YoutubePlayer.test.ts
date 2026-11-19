import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { AppState, WatchParty } from '../lib';
import YoutubePlayer from './YoutubePlayer.svelte';

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
    return { youtubeStream: vi.fn().mockResolvedValue(stream) } as unknown as AppState;
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

    it('shows a retry when the stream cannot be resolved', async () => {
        const app = makeApp(null);
        render(YoutubePlayer, { app, party: PARTY, selfId: 'node-1', onPublish: () => {} });

        await fireEvent.click(screen.getByRole('button', { name: 'Play video' }));

        await waitFor(() => expect(screen.getByText('Video unavailable.')).toBeTruthy());
        expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
    });
});

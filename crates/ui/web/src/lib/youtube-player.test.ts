import { describe, expect, it, vi } from 'vitest';

import {
    formatPlayerTime,
    listenToPlayer,
    playerUrl,
    readPlayerMessage,
    sendCommand,
    YOUTUBE_ORIGIN,
    YOUTUBE_ORIGINS,
    YOUTUBE_WEB_ORIGIN,
} from './youtube-player';

describe('youtube player protocol', () => {
    it('builds an iframe url with the parent origin', () => {
        const url = new URL(playerUrl('dQw4w9WgXcQ', 'http://127.0.0.1:5173'));
        expect(url.origin).toBe(YOUTUBE_ORIGIN);
        expect(url.pathname).toBe('/embed/dQw4w9WgXcQ');
        expect(url.searchParams.get('enablejsapi')).toBe('1');
        expect(url.searchParams.get('controls')).toBe('0');
        expect(url.searchParams.get('origin')).toBe('http://127.0.0.1:5173');
    });

    it('supports a fallback embed host', () => {
        const url = new URL(
            playerUrl('dQw4w9WgXcQ', 'http://127.0.0.1:5173', false, YOUTUBE_WEB_ORIGIN),
        );
        expect(url.origin).toBe(YOUTUBE_WEB_ORIGIN);
        expect(url.pathname).toBe('/embed/dQw4w9WgXcQ');
    });

    it('reads playback from infoDelivery messages', () => {
        const frame = { contentWindow: {} } as unknown as HTMLIFrameElement;
        const event = {
            origin: YOUTUBE_ORIGIN,
            source: frame.contentWindow,
            data: JSON.stringify({
                event: 'infoDelivery',
                info: { currentTime: 12.5, duration: 90, playerState: 1 },
            }),
        } as MessageEvent;

        expect(readPlayerMessage(event, frame)).toEqual({
            currentTime: 12.5,
            duration: 90,
            playing: true,
            ready: true,
        });
    });

    it('accepts playback messages from www.youtube.com', () => {
        const frame = { contentWindow: {} } as unknown as HTMLIFrameElement;
        const event = {
            origin: 'https://www.youtube.com',
            source: frame.contentWindow,
            data: JSON.stringify({
                event: 'infoDelivery',
                info: { currentTime: 4, duration: 120, playerState: 2 },
            }),
        } as MessageEvent;

        expect(readPlayerMessage(event, frame)).toEqual({
            currentTime: 4,
            duration: 120,
            playing: false,
            ready: true,
        });
    });

    it('posts commands and listening requests to both YouTube origins', () => {
        const postMessage = vi.fn();
        const frame = {
            contentWindow: { postMessage },
        } as unknown as HTMLIFrameElement;

        sendCommand(frame, 'playVideo');
        listenToPlayer(frame, 'ticonnect-player');

        expect(postMessage).toHaveBeenCalledTimes(4);
        for (const [index, call] of postMessage.mock.calls.entries()) {
            expect(call[1]).toBe(YOUTUBE_ORIGINS[index % YOUTUBE_ORIGINS.length]);
        }
    });

    it('ignores messages from other origins', () => {
        const frame = { contentWindow: {} } as unknown as HTMLIFrameElement;
        const event = {
            origin: 'https://evil.example',
            source: frame.contentWindow,
            data: JSON.stringify({ event: 'infoDelivery', info: {} }),
        } as MessageEvent;
        expect(readPlayerMessage(event, frame)).toBeNull();
    });

    it('formats player time', () => {
        expect(formatPlayerTime(0)).toBe('0:00');
        expect(formatPlayerTime(65)).toBe('1:05');
        expect(formatPlayerTime(3725)).toBe('1:02:05');
    });
});

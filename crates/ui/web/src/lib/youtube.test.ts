import { describe, expect, it } from 'vitest';

import { firstVideoId, parseVideoId } from './youtube';

describe('youtube', () => {
    it('parses supported URL shapes', () => {
        expect(parseVideoId('https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
        expect(parseVideoId('https://youtu.be/dQw4w9WgXcQ?t=4')).toBe('dQw4w9WgXcQ');
        expect(parseVideoId('https://www.youtube.com/shorts/dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ');
    });

    it('finds a video in text and rejects foreign hosts', () => {
        expect(firstVideoId('watch https://youtu.be/dQw4w9WgXcQ, now')).toBe('dQw4w9WgXcQ');
        expect(parseVideoId('https://example.com/watch?v=dQw4w9WgXcQ')).toBeNull();
    });
});

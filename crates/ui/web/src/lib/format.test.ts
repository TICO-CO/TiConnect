import { describe, expect, it } from 'vitest';

import {
    fileSize,
    fingerprintLabel,
    initials,
    preview,
    relativeSeen,
    sessionClock,
    shortName,
} from './format';

describe('format', () => {
    it('builds monograms', () => {
        expect(initials('MIKA-PC')).toBe('MP');
        expect(initials('Studio Imac')).toBe('SI');
        expect(initials('')).toBe('··');
    });

    it('falls back to the node id for empty names', () => {
        expect(shortName({ display_name: '  ', id: 'node-1', fingerprint: 'abc' })).toBe('node-1');
        expect(shortName({ display_name: 'Nina', id: 'node-1', fingerprint: 'abc' })).toBe('Nina');
    });

    it('formats fingerprints', () => {
        expect(fingerprintLabel('b371f09a2c5d4e6f8190')).toBe('B3:71F0…8190');
    });

    it('formats sizes and durations', () => {
        expect(fileSize(1_990_000)).toBe('1.9 MB');
        expect(sessionClock(252)).toBe('00:04:12');
        expect(sessionClock(3_661)).toBe('01:01:01');
    });

    it('previews text without whitespace noise', () => {
        expect(preview('  hello   there  ')).toBe('hello there');
    });

    it('describes presence', () => {
        expect(relativeSeen(0, true)).toBe('Online now');
        expect(relativeSeen(0, false)).toBe('Offline');
    });
});

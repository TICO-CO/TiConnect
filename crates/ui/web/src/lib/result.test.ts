import { describe, expect, it } from 'vitest';

import { attempt, attemptAsync } from './result';

describe('attempt', () => {
    it('returns the operation value', () => {
        const [value, err] = attempt(() => 42);
        expect(err).toBeNull();
        expect(value).toBe(42);
    });

    it('returns the thrown error as a value', () => {
        const [value, err] = attempt(() => JSON.parse('{'));
        expect(err).toBeInstanceOf(SyntaxError);
        expect(value).toBeNull();
    });
});

describe('attemptAsync', () => {
    it('returns the resolved operation value', async () => {
        const [value, err] = await attemptAsync(async () => 42);
        expect(err).toBeNull();
        expect(value).toBe(42);
    });

    it('returns the rejected error as a value', async () => {
        const [value, err] = await attemptAsync(() =>
            Promise.reject(new SyntaxError('invalid json')),
        );
        expect(err).toBeInstanceOf(SyntaxError);
        expect(value).toBeNull();
    });
});

import { describe, expect, it } from 'vitest';

import { attempt, attemptAsync, err, ok } from './result';

describe('ok', () => {
    it('wraps a value in a successful result', () => {
        const result = ok(42);
        expect(result.ok).toBe(true);
        if (!result.ok) {
            return;
        }
        expect(result.value).toBe(42);
    });
});

describe('err', () => {
    it('wraps an error in a failed result', () => {
        const result = err(new SyntaxError('invalid json'));
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toBeInstanceOf(SyntaxError);
    });
});

describe('attempt', () => {
    it('returns the operation value', () => {
        const result = attempt(() => 42);
        expect(result.ok).toBe(true);
        if (!result.ok) {
            return;
        }
        expect(result.value).toBe(42);
    });

    it('returns the thrown error as a value', () => {
        const result = attempt(() => JSON.parse('{'));
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toBeInstanceOf(SyntaxError);
    });
});

describe('attemptAsync', () => {
    it('returns the resolved operation value', async () => {
        const result = await attemptAsync(async () => 42);
        expect(result.ok).toBe(true);
        if (!result.ok) {
            return;
        }
        expect(result.value).toBe(42);
    });

    it('returns the rejected error as a value', async () => {
        const result = await attemptAsync(() => Promise.reject(new SyntaxError('invalid json')));
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toBeInstanceOf(SyntaxError);
    });
});

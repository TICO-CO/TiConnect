import { describe, expect, it } from 'vitest';

import { activationRoute, parseRoute, routeHash } from './routes';

describe('routes', () => {
    it('parses the designed screens', () => {
        expect(parseRoute('#/chat/user-1')).toEqual({ kind: 'chat', id: 'user-1' });
        expect(parseRoute('#/user/user-1')).toEqual({ kind: 'profile', id: 'user-1' });
        expect(parseRoute('#/settings/media')).toEqual({
            kind: 'settings',
            section: 'media',
        });
        expect(parseRoute('#/call/user-1')).toEqual({ kind: 'call', id: 'user-1' });
        expect(parseRoute('#/remote/user-1')).toEqual({ kind: 'remote', id: 'user-1' });
        expect(parseRoute('#/watch-party')).toEqual({ kind: 'watch-party' });
    });

    it('falls back to the empty chat route', () => {
        expect(parseRoute('')).toEqual({ kind: 'chat', id: '' });
        expect(parseRoute('#/settings/bogus')).toEqual({
            kind: 'settings',
            section: 'user',
        });
    });

    it('round-trips hashes', () => {
        const routes = [
            { kind: 'chat' as const, id: 'user-1' },
            { kind: 'profile' as const, id: 'user-1' },
            { kind: 'settings' as const, section: 'calls' as const },
            { kind: 'call' as const, id: 'user-1' },
            { kind: 'remote' as const, id: 'user-1' },
            { kind: 'watch-party' as const },
        ];
        routes.forEach((route) => expect(parseRoute(routeHash(route))).toEqual(route));
    });

    it('maps activation views', () => {
        expect(activationRoute('settings')).toEqual({ kind: 'settings', section: 'user' });
        expect(activationRoute('remote', 'user-1')).toEqual({ kind: 'remote', id: 'user-1' });
        expect(activationRoute('watch-party')).toEqual({ kind: 'watch-party' });
        expect(activationRoute('chat', 'user-1')).toEqual({ kind: 'chat', id: 'user-1' });
    });
});

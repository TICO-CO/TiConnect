import { describe, expect, it } from 'vitest';

import { BridgeClient, resolveBridge } from './bridge';

describe('resolveBridge', () => {
    it('uses the page origin and token in production', () => {
        const bridgeInfo = resolveBridge(new URL('http://127.0.0.1:41000/#token=abc123'));
        expect(bridgeInfo.httpBaseUrl).toBe('http://127.0.0.1:41000');
        expect(bridgeInfo.webSocketUrl).toBe('ws://127.0.0.1:41000/ws');
        expect(bridgeInfo.token).toBe('abc123');
        expect(bridgeInfo.development).toBe(false);
    });

    it('uses the bridge query parameter in dev', () => {
        const bridgeInfo = resolveBridge(new URL('http://127.0.0.1:5173/?bridge=127.0.0.1:41000'));
        expect(bridgeInfo.httpBaseUrl).toBe('http://127.0.0.1:41000');
        expect(bridgeInfo.webSocketUrl).toBe('ws://127.0.0.1:41000/ws');
        expect(bridgeInfo.token).toBeNull();
        expect(bridgeInfo.development).toBe(true);
    });

    it('accepts a full bridge url', () => {
        const bridgeInfo = resolveBridge(
            new URL('http://127.0.0.1:5173/?bridge=http://127.0.0.1:42000/'),
        );
        expect(bridgeInfo.httpBaseUrl).toBe('http://127.0.0.1:42000');
        expect(bridgeInfo.webSocketUrl).toBe('ws://127.0.0.1:42000/ws');
    });
});

describe('BridgeClient', () => {
    it('returns a not connected result before connecting', async () => {
        const client = new BridgeClient({
            httpBaseUrl: 'http://127.0.0.1:41000',
            webSocketUrl: 'ws://127.0.0.1:41000/ws',
            token: null,
            development: false,
        });
        const [result, failure] = await client.request('bootstrap');
        expect(failure).toEqual({
            type: 'NOT_CONNECTED',
            message: 'bridge is not connected',
        });
        expect(result).toBeNull();
    });
});

import { describe, expect, it, vi } from 'vitest';

import { BridgeClient, resolveBridge } from './bridge';
import { ok } from './result';

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
        const result = await client.request('bootstrap');
        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error).toEqual({
            type: 'NOT_CONNECTED',
            message: 'bridge is not connected',
        });
    });
});

describe('BridgeClient downloads', () => {
    it('retries a transient failure with a fresh ticket', async () => {
        const client = new BridgeClient({
            httpBaseUrl: 'http://127.0.0.1:41000',
            webSocketUrl: 'ws://127.0.0.1:41000/ws',
            token: null,
            development: false,
        });
        let ticket = 0;
        const request = vi.fn(async () =>
            ok({ url: `http://127.0.0.1:41000/attachment/hash?ticket=${++ticket}` }),
        );
        (client as unknown as { request: typeof request }).request = request;
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(new Response('', { status: 502 }))
            .mockResolvedValueOnce(new Response(new Blob(['image']), { status: 200 }));
        vi.stubGlobal('fetch', fetchMock);

        const result = await client.downloadAttachment('hash');

        expect(result.ok).toBe(true);
        if (!result.ok) {
            return;
        }
        expect(result.value).toBe('blob:test');
        expect(request).toHaveBeenCalledTimes(2);
        expect(fetchMock.mock.calls[0]?.[0]).not.toBe(fetchMock.mock.calls[1]?.[0]);
    });

    it('does not retry forbidden downloads', async () => {
        const client = new BridgeClient({
            httpBaseUrl: 'http://127.0.0.1:41000',
            webSocketUrl: 'ws://127.0.0.1:41000/ws',
            token: null,
            development: false,
        });
        const request = vi.fn(async () =>
            ok({ url: 'http://127.0.0.1:41000/attachment/hash?ticket=1' }),
        );
        (client as unknown as { request: typeof request }).request = request;
        const fetchMock = vi.fn().mockResolvedValue(new Response('', { status: 403 }));
        vi.stubGlobal('fetch', fetchMock);

        const result = await client.downloadAttachment('hash');

        expect(result.ok).toBe(false);
        if (result.ok) {
            return;
        }
        expect(result.error.message).toBe('download request failed (403)');
        expect(request).toHaveBeenCalledTimes(1);
        expect(fetchMock).toHaveBeenCalledTimes(1);
    });
});

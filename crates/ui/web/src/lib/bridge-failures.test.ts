import { describe, expect, it } from 'vitest';

import { BridgeFailures } from './bridge-failures';

describe('BridgeFailures', () => {
    it('constructs authentication failures', () => {
        expect(BridgeFailures.authenticationFailed('denied')).toEqual({
            type: 'AUTHENTICATION_FAILED',
            message: 'denied',
        });
    });

    it('constructs connection failures', () => {
        expect(BridgeFailures.connectionFailed('offline')).toEqual({
            type: 'CONNECTION_FAILED',
            message: 'offline',
        });
    });

    it('constructs closed connection failures', () => {
        expect(BridgeFailures.connectionClosed()).toEqual({
            type: 'CONNECTION_CLOSED',
            message: 'bridge connection closed',
        });
    });

    it('constructs malformed message failures', () => {
        expect(BridgeFailures.malformedMessage()).toEqual({
            type: 'MALFORMED_MESSAGE',
            message: 'bridge sent a malformed message',
        });
    });

    it('constructs not connected failures', () => {
        expect(BridgeFailures.notConnected()).toEqual({
            type: 'NOT_CONNECTED',
            message: 'bridge is not connected',
        });
    });

    it('constructs request failures', () => {
        expect(BridgeFailures.requestFailed('rejected')).toEqual({
            type: 'REQUEST_FAILED',
            message: 'rejected',
        });
    });
});

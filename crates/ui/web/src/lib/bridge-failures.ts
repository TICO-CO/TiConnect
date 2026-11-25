export type BridgeFailure =
    | { type: 'AUTHENTICATION_FAILED'; message: string }
    | { type: 'CONNECTION_CLOSED'; message: string }
    | { type: 'CONNECTION_FAILED'; message: string }
    | { type: 'MALFORMED_MESSAGE'; message: string }
    | { type: 'NOT_CONNECTED'; message: string }
    | { type: 'REQUEST_FAILED'; message: string };

export class BridgeFailures {
    static authenticationFailed(message: string): BridgeFailure {
        return { type: 'AUTHENTICATION_FAILED', message };
    }

    static connectionClosed(): BridgeFailure {
        return { type: 'CONNECTION_CLOSED', message: 'bridge connection closed' };
    }

    static connectionFailed(message: string): BridgeFailure {
        return { type: 'CONNECTION_FAILED', message };
    }

    static malformedMessage(): BridgeFailure {
        return { type: 'MALFORMED_MESSAGE', message: 'bridge sent a malformed message' };
    }

    static notConnected(): BridgeFailure {
        return { type: 'NOT_CONNECTED', message: 'bridge is not connected' };
    }

    static requestFailed(message: string): BridgeFailure {
        return { type: 'REQUEST_FAILED', message };
    }
}

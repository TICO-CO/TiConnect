import type { BridgeFailure } from './bridge-failures';
import { BridgeFailures } from './bridge-failures';
import type { Result } from './result';
import { attempt } from './result';
import type { RpcResponse, WebSocketEvent } from './types';
import { BridgeMessages } from './types';

export interface BridgeInfo {
    httpBaseUrl: string;
    webSocketUrl: string;
    token: string | null;
    development: boolean;
}

function normalizeBaseUrl(value: string): string {
    const withScheme = value.includes('://') ? value : `http://${value}`;
    return withScheme.replace(/\/+$/, '');
}

export function resolveBridge(pageLocation: URL): BridgeInfo {
    const queryParameters = new URLSearchParams(pageLocation.search);
    const bridgeOverride = queryParameters.get('bridge');
    const httpBaseUrl = bridgeOverride ? normalizeBaseUrl(bridgeOverride) : pageLocation.origin;
    const hashParameters = new URLSearchParams(pageLocation.hash.replace(/^#/, ''));
    return {
        httpBaseUrl,
        webSocketUrl: `${httpBaseUrl.replace(/^http/, 'ws')}/ws`,
        token: hashParameters.get('token'),
        development: Boolean(bridgeOverride) || pageLocation.port === '5173',
    };
}

type PendingRequest = {
    succeed: (value: unknown) => void;
    fail: (failure: BridgeFailure) => void;
};

export class BridgeClient {
    private socket: WebSocket | null = null;
    private pendingRequests = new Map<number, PendingRequest>();
    private eventListeners = new Set<(event: WebSocketEvent) => void>();
    private nextId = 1;
    private ready: Promise<Result<void, BridgeFailure>> | null = null;
    private closeRequested = false;

    constructor(private readonly bridgeInfo: BridgeInfo) {}

    onEvent(listener: (event: WebSocketEvent) => void): () => void {
        this.eventListeners.add(listener);
        return () => this.eventListeners.delete(listener);
    }

    connect(): Promise<Result<void, BridgeFailure>> {
        if (this.ready) {
            return this.ready;
        }
        this.closeRequested = false;
        const [socket, socketError] = attempt<WebSocket>(
            () => new WebSocket(this.bridgeInfo.webSocketUrl),
        );
        if (socketError) {
            const message =
                socketError instanceof Error ? socketError.message : String(socketError);
            return Promise.resolve([null, BridgeFailures.connectionFailed(message)]);
        }
        if (socket === null) {
            return Promise.resolve([
                null,
                BridgeFailures.connectionFailed('bridge socket unavailable'),
            ]);
        }

        this.socket = socket;
        this.ready = new Promise<Result<void, BridgeFailure>>((resolve) => {
            let settled = false;

            const settleFailure = (failure: BridgeFailure): void => {
                if (settled) {
                    return;
                }
                settled = true;
                resolve([null, failure]);
            };

            const settleSuccess = (): void => {
                if (settled) {
                    return;
                }
                settled = true;
                resolve([null, null]);
            };

            socket.addEventListener('open', () => {
                const [, sendError] = attempt(() =>
                    socket.send(
                        JSON.stringify({
                            type: 'auth',
                            ...(this.bridgeInfo.token ? { token: this.bridgeInfo.token } : {}),
                        }),
                    ),
                );
                if (sendError) {
                    const message =
                        sendError instanceof Error ? sendError.message : String(sendError);
                    settleFailure(BridgeFailures.connectionFailed(message));
                }
            });
            socket.addEventListener('message', (messageEvent) => {
                const value = this.parse(messageEvent.data);
                if (value === null) {
                    settleFailure(BridgeFailures.malformedMessage());
                    return;
                }
                if (BridgeMessages.isReply(value)) {
                    this.handleReply(value);
                    return;
                }
                if (!BridgeMessages.isEvent(value)) {
                    settleFailure(BridgeFailures.malformedMessage());
                    return;
                }
                if (value.type === 'ready') {
                    settleSuccess();
                    return;
                }
                if (value.type === 'error') {
                    const message = String(value.message ?? 'bridge authentication failed');
                    settleFailure(BridgeFailures.authenticationFailed(message));
                    return;
                }
                this.dispatch(value);
            });
            socket.addEventListener('error', () =>
                settleFailure(BridgeFailures.connectionFailed('bridge connection failed')),
            );
            socket.addEventListener('close', () => {
                this.socket = null;
                this.ready = null;
                this.pendingRequests.forEach((pendingRequest) =>
                    pendingRequest.fail(BridgeFailures.connectionClosed()),
                );
                this.pendingRequests.clear();
                settleFailure(BridgeFailures.connectionClosed());
                if (this.closeRequested) {
                    return;
                }
                this.dispatch({ type: 'disconnected' });
            });
        });
        return this.ready;
    }

    request<T>(
        method: string,
        parameters: Record<string, unknown> = {},
    ): Promise<Result<T, BridgeFailure>> {
        const socket = this.socket;
        if (socket === null || socket.readyState !== WebSocket.OPEN) {
            return Promise.resolve([null, BridgeFailures.notConnected()]);
        }
        const id = this.nextId++;
        return new Promise<Result<T, BridgeFailure>>((resolve) => {
            this.pendingRequests.set(id, {
                succeed: (value) => resolve([value as T, null]),
                fail: (failure) => resolve([null, failure]),
            });
            const [, sendError] = attempt(() =>
                socket.send(JSON.stringify({ id, method, params: parameters })),
            );
            if (!sendError) {
                return;
            }
            this.pendingRequests.delete(id);
            const message = sendError instanceof Error ? sendError.message : String(sendError);
            resolve([null, BridgeFailures.requestFailed(message)]);
        });
    }

    close(): void {
        this.closeRequested = true;
        this.socket?.close();
        this.socket = null;
        this.ready = null;
    }

    private parse(rawData: unknown): WebSocketEvent | RpcResponse | null {
        if (typeof rawData !== 'string') {
            return null;
        }
        const [parsed, parseError] = attempt<unknown>(() => JSON.parse(rawData));
        if (parseError) {
            return null;
        }
        if (parsed === null) {
            return null;
        }
        if (BridgeMessages.isEvent(parsed) || BridgeMessages.isReply(parsed)) {
            return parsed;
        }
        return null;
    }

    private dispatch(event: WebSocketEvent): void {
        this.eventListeners.forEach((listener) => listener(event));
    }

    private handleReply(reply: RpcResponse): void {
        const requestId = Number(reply.id);
        const pendingRequest = this.pendingRequests.get(requestId);
        if (pendingRequest === undefined) {
            return;
        }
        this.pendingRequests.delete(requestId);
        if (reply.ok) {
            pendingRequest.succeed(reply.result ?? null);
            return;
        }
        const message = reply.error?.message ?? 'bridge request failed';
        pendingRequest.fail(BridgeFailures.requestFailed(message));
    }
}

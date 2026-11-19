import type { BridgeFailure } from './bridge-failures';
import { BridgeFailures } from './bridge-failures';
import type { Result } from './result';
import { attempt, attemptAsync } from './result';
import type { RemoteTicket, RpcResponse, WebSocketEvent } from './types';
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

    get baseUrl(): string {
        return this.bridgeInfo.httpBaseUrl;
    }

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

    async downloadAvatar(fingerprint: string): Promise<Result<string, BridgeFailure>> {
        return this.download(
            'avatar_ticket',
            { fingerprint },
            `/avatar/${encodeURIComponent(fingerprint)}`,
        );
    }

    async downloadAttachment(hash: string): Promise<Result<string, BridgeFailure>> {
        return this.download(
            'attachment_ticket',
            { hash },
            `/attachment/${encodeURIComponent(hash)}`,
        );
    }

    async uploadAttachment(userId: string, file: File): Promise<Result<string, BridgeFailure>> {
        const path = `/attachment?user_id=${encodeURIComponent(userId)}&name=${encodeURIComponent(file.name)}&mime=${encodeURIComponent(file.type)}`;
        const [value, failure] = await this.upload(path, file);
        if (failure) {
            return [null, failure];
        }
        if (typeof value !== 'object' || value === null || !('message_id' in value)) {
            return [null, BridgeFailures.requestFailed('attachment upload returned no message id')];
        }
        const messageId = value.message_id;
        if (typeof messageId !== 'string') {
            return [null, BridgeFailures.requestFailed('attachment upload returned no message id')];
        }
        return [messageId, null];
    }

    async uploadAvatar(file: File): Promise<Result<void, BridgeFailure>> {
        const [, failure] = await this.upload('/avatar', file);
        return [null, failure];
    }

    private async download(
        method: string,
        parameters: Record<string, unknown>,
        path: string,
    ): Promise<Result<string, BridgeFailure>> {
        const deadline = Date.now() + 15_000;
        let delay = 200;
        let lastStatus = 0;
        for (;;) {
            const [ticket, ticketFailure] = await this.request<RemoteTicket>(method, parameters);
            if (ticketFailure) {
                return [null, ticketFailure];
            }
            if (ticket === null || typeof ticket.url !== 'string') {
                return [
                    null,
                    BridgeFailures.requestFailed('bridge did not return a download ticket'),
                ];
            }
            const [response, fetchFailure] = await attemptAsync(() =>
                fetch(ticket.url, { cache: 'no-store' }),
            );
            lastStatus = response?.status ?? 0;
            if (!fetchFailure && response !== null && response.ok) {
                const [blob, blobFailure] = await attemptAsync(() => response.blob());
                if (blobFailure || blob === null) {
                    return [null, BridgeFailures.requestFailed('download response was empty')];
                }
                const [url, urlFailure] = attempt(() => URL.createObjectURL(blob));
                if (urlFailure || url === null) {
                    return [null, BridgeFailures.requestFailed('download could not be opened')];
                }
                return [url, null];
            }
            if (!shouldRetryDownload(fetchFailure !== null, lastStatus)) {
                return [null, downloadFailure(lastStatus)];
            }
            if (Date.now() >= deadline) {
                return [null, downloadFailure(lastStatus, true)];
            }
            await wait(delay);
            delay = Math.min(delay * 2, 1_000);
        }
    }

    private async upload(path: string, file: File): Promise<Result<unknown, BridgeFailure>> {
        const headers: Record<string, string> = { 'Content-Type': 'application/octet-stream' };
        if (this.bridgeInfo.token) {
            headers.Authorization = `Bearer ${this.bridgeInfo.token}`;
        }
        const [response, fetchFailure] = await attemptAsync(() =>
            fetch(`${this.bridgeInfo.httpBaseUrl}${path}`, {
                method: 'POST',
                headers,
                body: file,
            }),
        );
        if (fetchFailure) {
            return [null, BridgeFailures.requestFailed('upload request failed')];
        }
        if (response === null || !response.ok) {
            return [null, BridgeFailures.requestFailed('upload request failed')];
        }
        const [value, parseFailure] = await attemptAsync<unknown>(() => response.json());
        if (parseFailure) {
            return [null, BridgeFailures.requestFailed('upload response was malformed')];
        }
        return [value, null];
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

function shouldRetryDownload(fetchFailed: boolean, status: number): boolean {
    if (fetchFailed) {
        return true;
    }
    return status !== 401 && status !== 403;
}

function downloadFailure(status: number, timedOut = false): BridgeFailure {
    if (timedOut) {
        return BridgeFailures.requestFailed('download request timed out');
    }
    if (status > 0) {
        return BridgeFailures.requestFailed(`download request failed (${status})`);
    }
    return BridgeFailures.requestFailed('download request failed');
}

function wait(milliseconds: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

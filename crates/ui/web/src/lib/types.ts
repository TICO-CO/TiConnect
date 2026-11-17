export interface Identity {
    node_id: string;
    fingerprint: string;
    display_name: string;
    avatar_hash: string;
}

export interface Status {
    transport_state: string;
    active_sessions: number;
    outbox_depth: number;
    peer_count: number;
}

export interface Bootstrap {
    identity: Identity;
    status: Status;
    settings: Record<string, string>;
    appearance: string;
    autostart: boolean;
    version: string;
    online: boolean;
}

export interface WebSocketEvent {
    type: string;
    [key: string]: unknown;
}

export interface RpcError {
    code: string;
    message: string;
}

export interface RpcResponse<T = unknown> {
    id: number | string | null;
    ok: boolean;
    result?: T;
    error?: RpcError;
}

export class BridgeMessages {
    private static isRecord(value: unknown): value is Record<string, unknown> {
        return typeof value === 'object' && value !== null;
    }

    static isEvent(value: unknown): value is WebSocketEvent {
        return BridgeMessages.isRecord(value) && typeof value.type === 'string';
    }

    static isReply(value: unknown): value is RpcResponse {
        return BridgeMessages.isRecord(value) && 'id' in value && typeof value.ok === 'boolean';
    }
}

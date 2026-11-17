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
    user_count: number;
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

export type UserPolicy = 'accept' | 'mute' | 'ignore';

export interface User {
    id: string;
    fingerprint: string;
    display_name: string;
    avatar_hash: string;
    policy: UserPolicy;
    trusted_at: number;
    last_seen: number;
    connected: boolean;
}

export interface UserDetails {
    user: User;
    addresses: string;
    port: number;
    source: string;
    mac: string;
    os: string;
    desktop: string;
    display_server: string;
    app_version: string;
}

export interface Message {
    id: string;
    conversation_id: string;
    user_id: string;
    sender_id: string;
    local_sender: boolean;
    seq: number;
    sent_at: number;
    received_at: number;
    body: string;
    attachment_hash: string;
    attachment_name: string;
    attachment_size: number;
    attachment_mime: string;
    attachment_path: string;
    state: string;
    notify: boolean;
}

export interface ConversationSummary {
    conversation_id: string;
    user_id: string;
    last_message_id: string;
    last_sender_id: string;
    last_body: string;
    last_sent_at: number;
    unread_count: number;
}

export interface WatchParty {
    party_id: string;
    user_id: string;
    video_id: string;
    role: string;
    state: string;
    position_ms: number;
    playing: boolean;
    revision: number;
    actor: string;
    updated_at: number;
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

export interface Identity {
    node_id: string;
    fingerprint: string;
    display_name: string;
    avatar_hash: string;
    created_at: number;
}

export interface Status {
    transport_state: string;
    active_sessions: number;
    outbox_depth: number;
    user_count: number;
}

export interface NetworkStatus {
    state: string;
    confidence: string;
    hint: string;
    interfaces: string;
    interface_count: number;
    transport_port: number;
    transport_bound: boolean;
    mdns_active: boolean;
    broadcast_active: boolean;
    candidate_count: number;
    active_sessions: number;
    beacons_sent: number;
    beacons_received: number;
    beacon_send_errors: number;
    dropped_packets: number;
    last_beacon_sent_at: number;
    last_beacon_received_at: number;
    last_candidate_at: number;
    last_session_at: number;
    last_failure_at: number;
    transport_failures: number;
}

export interface Bootstrap {
    identity: Identity;
    status: Status;
    network: NetworkStatus;
    settings: Record<string, string>;
    appearance: string;
    autostart: boolean;
    version: string;
    online: boolean;
}

export type UserPolicy = 'accept' | 'mute' | 'ignore';

export interface StorageUsage {
    message_count: number;
    attachment_count: number;
    attachment_bytes: number;
    database_bytes: number;
}

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
    reply_to: string;
    attachment_hash: string;
    attachment_name: string;
    attachment_size: number;
    attachment_mime: string;
    attachment_path: string;
    state: string;
    edited_at: number;
    deleted_at: number;
    notify: boolean;
}

export interface ConversationSummary {
    conversation_id: string;
    user_id: string;
    last_message_id: string;
    last_sender_id: string;
    last_body: string;
    last_deleted: boolean;
    last_sent_at: number;
    unread_count: number;
}

export interface WatchParty {
    party_id: string;
    host_id: string;
    video_id: string;
    role: string;
    state: string;
    participants: string[];
    invited: string[];
    position_ms: number;
    playing: boolean;
    revision: number;
    actor: string;
    updated_at: number;
}

export interface YoutubeStream {
    video_id: string;
    url: string;
}

export interface YoutubePreview {
    video_id: string;
    title: string;
    channel: string;
    description: string;
    thumbnail_url: string;
    thumbnail_data: string | null;
    duration_seconds: number;
}

export interface Call {
    id: string;
    user_id: string;
    direction: string;
    state: string;
    started_at: number;
    connected_at: number;
    input_muted: boolean;
    output_muted: boolean;
}

export interface CallMetrics {
    frames_sent: number;
    frames_received: number;
    packets_lost: number;
    bytes_sent: number;
    output_level: number;
}

export interface CallLog {
    id: string;
    user_id: string;
    direction: string;
    started_at: number;
    connected_at: number;
    ended_at: number;
    duration_secs: number;
    reason: string;
}

export interface AudioDevice {
    id: string;
    name: string;
    kind: string;
}

export interface RemoteSession {
    session_id: string;
    user_id: string;
    fingerprint: string;
    role: string;
    state: string;
    indicator: boolean;
    input_allowed: boolean;
    expires_at: number;
}

export interface RemoteTicket {
    url: string;
    expires_at: number;
}

export interface BlockedUser {
    fingerprint: string;
    node_id: string;
    display_name: string;
    blocked_at: number;
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

export type Connection = 'connecting' | 'online' | 'offline';

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

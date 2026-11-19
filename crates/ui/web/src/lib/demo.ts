import type {
    Bootstrap,
    Call,
    ConversationSummary,
    Message,
    RemoteSession,
    User,
    UserDetails,
    WatchParty,
} from './types';

const now = Date.now();
const minute = 60_000;

export function demoUser(
    id: string,
    name: string,
    fingerprint: string,
    connected: boolean,
    policy: User['policy'] = 'accept',
): User {
    return {
        id,
        fingerprint,
        display_name: name,
        avatar_hash: '',
        policy,
        trusted_at: now - 28 * 86_400_000,
        last_seen: now - minute * 14,
        connected,
    };
}

export const DEMO_USERS: User[] = [
    demoUser('01M7DEMO000000000000000001', 'MIKA-PC', 'b371f09a2c5d4e6f8190', true),
    demoUser('01M7DEMO000000000000000002', 'STUDIO-IMAC', '7fd1a4c93b2e0d8a0123', true),
    demoUser('01M7DEMO000000000000000003', 'NINA-LAPTOP', '4e2c8a17d9b3f605e8a1', true),
    demoUser('01M7DEMO000000000000000004', 'GUEST-01', '9cb52d1047f6a3e2b8d0', true, 'mute'),
    demoUser('01M7DEMO000000000000000005', 'OLD-LATITUDE', '1a4e9f8276cd05b3a2e7', false),
];

export const DEMO_BOOTSTRAP: Bootstrap = {
    identity: {
        node_id: '01M7DEMO000000000000000001',
        fingerprint: 'b371f09a2c5d4e6f8190',
        display_name: 'MIKA-PC',
        avatar_hash: '',
        created_at: now - 90 * 86_400_000,
    },
    status: {
        transport_state: 'connected',
        active_sessions: 4,
        outbox_depth: 0,
        user_count: 5,
    },
    settings: {
        'identity.display_name': 'MIKA-PC',
        'dnd.enabled': 'false',
        'read_receipts.enabled': 'true',
        'typing.enabled': 'true',
        'overlay.enabled': 'true',
        'overlay.timeout_secs': '5',
        'calls.timeout_secs': '30',
        'calls.input_device': '',
        'calls.output_device': '',
        'ringtone.path': '',
        'remote.session_timeout_secs': '0',
        'remote.request_timeout_secs': '300',
        'video.bitrate_kbps': '2500',
        'video.fps': '30',
        'video.keyframe_interval': '60',
        'outbox.ttl_days': '7',
        'attachments.max_size_mib': '2048',
        'appearance.color_scheme': 'dark',
        'discovery.enabled': 'true',
        'discovery.mdns': 'true',
        'discovery.broadcast': 'true',
        'discovery.beacon_interval_secs': '5',
    },
    appearance: 'dark',
    autostart: true,
    version: '0.0.6',
    online: true,
};

export const DEMO_SUMMARIES: ConversationSummary[] = [
    {
        conversation_id: 'conv-mika',
        user_id: DEMO_USERS[0]?.id ?? '',
        last_message_id: 'm6',
        last_sender_id: DEMO_USERS[0]?.id ?? '',
        last_body: 'Benchmark finished — check the log',
        last_sent_at: now - minute * 2,
        unread_count: 2,
    },
    {
        conversation_id: 'conv-studio',
        user_id: DEMO_USERS[1]?.id ?? '',
        last_message_id: 'm7',
        last_sender_id: DEMO_USERS[1]?.id ?? '',
        last_body: 'In a call · 04:12',
        last_sent_at: now - minute * 6,
        unread_count: 0,
    },
    {
        conversation_id: 'conv-nina',
        user_id: DEMO_USERS[2]?.id ?? '',
        last_message_id: 'm8',
        last_sender_id: DEMO_USERS[2]?.id ?? '',
        last_body: 'Address changed · 3 photos',
        last_sent_at: now - minute * 13,
        unread_count: 1,
    },
    {
        conversation_id: 'conv-guest',
        user_id: DEMO_USERS[3]?.id ?? '',
        last_message_id: 'm9',
        last_sender_id: DEMO_USERS[3]?.id ?? '',
        last_body: 'Muted · 12 messages',
        last_sent_at: now - minute * 46,
        unread_count: 12,
    },
    {
        conversation_id: 'conv-old',
        user_id: DEMO_USERS[4]?.id ?? '',
        last_message_id: 'm10',
        last_sender_id: DEMO_USERS[4]?.id ?? '',
        last_body: 'Offline since 14 Nov',
        last_sent_at: now - 3 * 86_400_000,
        unread_count: 0,
    },
];

function demoMessage(
    id: string,
    user: User,
    seq: number,
    body: string,
    localSender: boolean,
    offsetMinutes: number,
    state = 'read',
): Message {
    return {
        id,
        conversation_id: 'conv-mika',
        user_id: user.id,
        sender_id: user.id,
        local_sender: localSender,
        seq,
        sent_at: now - offsetMinutes * minute,
        received_at: now - offsetMinutes * minute,
        body,
        attachment_hash: '',
        attachment_name: '',
        attachment_size: 0,
        attachment_mime: '',
        attachment_path: '',
        state,
        notify: true,
    };
}

const demoPeer = DEMO_USERS[0] as User;

export const DEMO_MESSAGES: Message[] = [
    {
        ...demoMessage(
            'm1',
            demoPeer,
            1,
            'The LAN benchmark run finished. 41 users discovered in under a second, QUIC handshake at 24 ms on the second box.',
            false,
            6,
        ),
    },
    {
        ...demoMessage('m2', demoPeer, 2, '', false, 6),
        attachment_hash: 'b371f09a2c5d4e6f8190',
        attachment_name: 'lan-benchmark.txt',
        attachment_size: 1_990_000,
        attachment_mime: 'text/plain',
    },
    demoMessage(
        'm3',
        demoPeer,
        3,
        'This is the build walkthrough — I’ll start a watch party if you want.',
        false,
        5,
    ),
    demoMessage('m4', demoPeer, 4, 'https://www.youtube.com/watch?v=M7lc1UVf-VE', false, 5),
    demoMessage('m5', demoPeer, 5, 'Good. Keep that number in the release notes.', true, 3),
    demoMessage('m6', demoPeer, 6, 'I’ll join the party — send the invite.', true, 1),
];

export const DEMO_DETAILS: UserDetails = {
    user: demoPeer,
    addresses: '192.168.1.24',
    port: 45871,
    source: 'mdns',
    mac: 'not exposed',
    os: 'Linux · Ubuntu 24.04',
    desktop: 'GNOME 48',
    display_server: 'Wayland',
    app_version: 'TiConnect 0.0.6',
};

export const DEMO_PARTY: WatchParty = {
    party_id: 'party-demo',
    host_id: DEMO_BOOTSTRAP.identity.node_id,
    video_id: 'M7lc1UVf-VE',
    role: 'host',
    state: 'active',
    participants: [DEMO_BOOTSTRAP.identity.node_id, demoPeer.id],
    invited: [],
    position_ms: 372_000,
    playing: true,
    revision: 4,
    actor: demoPeer.id,
    updated_at: now - minute,
};

export const DEMO_CALL: Call = {
    id: 'call-demo',
    user_id: demoPeer.id,
    direction: 'outgoing',
    state: 'in_call',
    started_at: now - 252_000,
    connected_at: now - 248_000,
};

export const DEMO_REMOTE: RemoteSession = {
    session_id: 'remote-demo',
    user_id: DEMO_USERS[1]?.id ?? '',
    fingerprint: DEMO_USERS[1]?.fingerprint ?? '',
    role: 'host',
    state: 'active',
    indicator: true,
    expires_at: now + 300_000,
};

export interface DemoScreen {
    users: User[];
    bootstrap: Bootstrap;
    summaries: ConversationSummary[];
    messages: Message[];
    details: UserDetails;
    party: WatchParty | null;
    call: Call | null;
    remote: RemoteSession | null;
    route: string;
}

export function demoScreen(value: string): DemoScreen {
    const chat = value === '1' || value === 'chat' || value === '';
    const call = value === 'call';
    const viewer = value === 'remote-viewer';
    const remote = value === 'remote' || viewer;
    const party = value === 'watch-party';
    const user = value === 'profile';
    const overlay = value === 'overlay';
    const active = value === 'active';
    const route = call
        ? `#/call/${DEMO_USERS[0]?.id ?? ''}`
        : remote
          ? `#/remote/${DEMO_USERS[0]?.id ?? ''}`
          : party
            ? '#/watch-party'
            : user
              ? `#/user/${DEMO_USERS[0]?.id ?? ''}`
              : `#/chat/${DEMO_USERS[0]?.id ?? ''}`;
    const callSeed = overlay
        ? {
              ...DEMO_CALL,
              direction: 'incoming',
              state: 'ringing',
              started_at: now,
              connected_at: 0,
          }
        : active
          ? DEMO_CALL
          : call
            ? DEMO_CALL
            : null;
    const remoteSeed = overlay
        ? { ...DEMO_REMOTE, role: 'host', state: 'requested', indicator: false }
        : active
          ? {
                ...DEMO_REMOTE,
                role: 'host',
                state: 'active',
                indicator: true,
                expires_at: now + 79_000,
            }
          : remote
            ? {
                  ...DEMO_REMOTE,
                  role: viewer ? 'viewer' : 'host',
                  state: viewer ? 'requested' : 'active',
                  indicator: !viewer,
              }
            : null;
    const partySeed = overlay
        ? { ...DEMO_PARTY, role: 'guest', state: 'ringing', playing: false }
        : active
          ? DEMO_PARTY
          : party
            ? DEMO_PARTY
            : null;
    return {
        users: DEMO_USERS,
        bootstrap: DEMO_BOOTSTRAP,
        summaries: DEMO_SUMMARIES,
        messages: DEMO_MESSAGES,
        details: DEMO_DETAILS,
        party: partySeed,
        call: callSeed,
        remote: remoteSeed,
        route,
    };
}

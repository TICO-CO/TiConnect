export type SettingsSection =
    'user' | 'notifications' | 'calls' | 'audio' | 'remote' | 'media' | 'storage' | 'about';

export type Route =
    | { kind: 'chat'; id: string }
    | { kind: 'profile'; id: string }
    | { kind: 'settings'; section: SettingsSection }
    | { kind: 'call'; id: string }
    | { kind: 'remote'; id: string }
    | { kind: 'watch-party' };

export const SETTINGS_SECTIONS: ReadonlyArray<{
    id: SettingsSection;
    label: string;
}> = [
    { id: 'user', label: 'User' },
    { id: 'notifications', label: 'Notifications' },
    { id: 'calls', label: 'Calls' },
    { id: 'audio', label: 'Audio' },
    { id: 'remote', label: 'Remote desktop' },
    { id: 'media', label: 'Media' },
    { id: 'storage', label: 'Storage' },
    { id: 'about', label: 'About' },
];

function parseSettingsSection(value: string): SettingsSection {
    const match = SETTINGS_SECTIONS.find((section) => section.id === value);
    return match?.id ?? 'user';
}

export function parseRoute(hash: string): Route {
    const path = hash.replace(/^#\/?/, '');
    const [kind = '', first = '', second = ''] = path.split('/');
    if (kind === 'user' && first) {
        return { kind: 'profile', id: first };
    }
    if (kind === 'chat' && first) {
        return { kind: 'chat', id: first };
    }
    if (kind === 'settings') {
        return { kind: 'settings', section: parseSettingsSection(first) };
    }
    if (kind === 'call' && first) {
        return { kind: 'call', id: first };
    }
    if (kind === 'remote' && first) {
        return { kind: 'remote', id: first };
    }
    if (kind === 'watch-party') {
        return { kind: 'watch-party' };
    }
    if (first && second === '') {
        return { kind: 'chat', id: '' };
    }
    return { kind: 'chat', id: '' };
}

export function routeHash(route: Route): string {
    if (route.kind === 'profile') {
        return `#/user/${route.id}`;
    }
    if (route.kind === 'chat') {
        return route.id ? `#/chat/${route.id}` : '#/chat';
    }
    if (route.kind === 'settings') {
        return `#/settings/${route.section}`;
    }
    if (route.kind === 'call') {
        return `#/call/${route.id}`;
    }
    if (route.kind === 'remote') {
        return `#/remote/${route.id}`;
    }
    return '#/watch-party';
}

export function activationRoute(view: string, userId?: string): Route {
    if (view === 'settings') {
        return { kind: 'settings', section: 'user' };
    }
    if (view === 'remote' && userId) {
        return { kind: 'remote', id: userId };
    }
    if (view === 'watch-party') {
        return { kind: 'watch-party' };
    }
    if (userId) {
        return { kind: 'chat', id: userId };
    }
    return { kind: 'chat', id: '' };
}

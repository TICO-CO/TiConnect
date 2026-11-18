import { type BridgeInfo, resolveBridge } from './bridge';

export function demoParameter(): string | null {
    const query = window.location.search.replace(/^\?/, '');
    const entry = query.split('&').find((part) => part === 'demo' || part.startsWith('demo='));
    if (entry === undefined) {
        return null;
    }
    const [, value = ''] = entry.split('=');
    return decodeURIComponent(value);
}

export function bridgeFromWindow(): BridgeInfo {
    return resolveBridge(new URL(window.location.href));
}

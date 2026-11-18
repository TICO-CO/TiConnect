import { attempt } from './result';

const STORAGE_KEY = 'ticonnect.preferences';

export interface Preferences {
    userDescription: string;
    presence: string;
    sounds: boolean;
    autoplay: boolean;
    queueLimit: number;
    playerPosition: string;
    ignoreIncoming: boolean;
    dismissPlayer: string;
    partyControl: string;
    acceptInvites: boolean;
    storageRetention: number;
    previousAddresses: Record<string, string>;
}

export const DEFAULT_PREFERENCES: Preferences = {
    userDescription: '',
    presence: 'online',
    sounds: true,
    autoplay: true,
    queueLimit: 3,
    playerPosition: 'Bot-right, above message toasts',
    ignoreIncoming: false,
    dismissPlayer: 'Manual',
    partyControl: 'Everyone',
    acceptInvites: true,
    storageRetention: 30,
    previousAddresses: {},
};

export function loadPreferences(): Preferences {
    const [raw, readError] = attempt(() => localStorage.getItem(STORAGE_KEY));
    if (readError || raw === null) {
        return { ...DEFAULT_PREFERENCES };
    }
    const [parsed, parseError] = attempt<unknown>(() => JSON.parse(raw));
    if (parseError || typeof parsed !== 'object' || parsed === null) {
        return { ...DEFAULT_PREFERENCES };
    }
    return { ...DEFAULT_PREFERENCES, ...(parsed as Partial<Preferences>) };
}

export function savePreferences(preferences: Preferences): boolean {
    const [, writeError] = attempt(() =>
        localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)),
    );
    return writeError === null;
}

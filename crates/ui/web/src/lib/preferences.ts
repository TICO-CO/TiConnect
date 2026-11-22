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
    const rawResult = attempt(() => localStorage.getItem(STORAGE_KEY));
    if (!rawResult.ok || rawResult.value === null) {
        return { ...DEFAULT_PREFERENCES };
    }
    const raw = rawResult.value;
    const parsedResult = attempt<unknown>(() => JSON.parse(raw));
    if (!parsedResult.ok || typeof parsedResult.value !== 'object' || parsedResult.value === null) {
        return { ...DEFAULT_PREFERENCES };
    }
    return { ...DEFAULT_PREFERENCES, ...(parsedResult.value as Partial<Preferences>) };
}

export function savePreferences(preferences: Preferences): boolean {
    const writeResult = attempt(() =>
        localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)),
    );
    return writeResult.ok;
}

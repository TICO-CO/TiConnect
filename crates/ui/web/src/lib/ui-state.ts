export type StateTone = 'neutral' | 'info' | 'success' | 'warning' | 'error';

export type LoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'error';

export type LifecycleState = 'booting' | 'ready' | 'offline' | 'fatal';

export type SaveState = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

export interface RemoteOutcome {
    userId: string;
    role: string;
    reason: string;
    endedAt: number;
}

export interface PartyOutcome {
    partyId: string;
    hostId: string;
    reason: string;
    endedAt: number;
}

export interface ConfirmRequest {
    title: string;
    body: string;
    confirmLabel: string;
    cancelLabel: string;
    tone: StateTone;
    action: () => void | Promise<void>;
}

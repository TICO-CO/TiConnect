import { attempt } from './result';

export const YOUTUBE_ORIGIN = 'https://www.youtube-nocookie.com';
export const YOUTUBE_WEB_ORIGIN = 'https://www.youtube.com';
export const YOUTUBE_ORIGINS = [YOUTUBE_ORIGIN, YOUTUBE_WEB_ORIGIN] as const;

export interface PlayerSnapshot {
    currentTime: number;
    duration: number;
    playing: boolean;
    ready: boolean;
}

export function playerUrl(
    videoId: string,
    origin: string,
    autoplay = false,
    host = YOUTUBE_ORIGIN,
): string {
    const parameters = new URLSearchParams({
        enablejsapi: '1',
        controls: '0',
        rel: '0',
        autoplay: autoplay ? '1' : '0',
        origin,
    });
    return `${host}/embed/${videoId}?${parameters.toString()}`;
}

export function sendCommand(
    frame: HTMLIFrameElement | null,
    func: string,
    args: unknown[] = [],
): void {
    postToPlayer(frame, JSON.stringify({ event: 'command', func, args }));
}

export function listenToPlayer(frame: HTMLIFrameElement | null, id: string): void {
    postToPlayer(frame, JSON.stringify({ event: 'listening', id }));
}

function postToPlayer(frame: HTMLIFrameElement | null, message: string): void {
    for (const origin of YOUTUBE_ORIGINS) {
        frame?.contentWindow?.postMessage(message, origin);
    }
}

function messageData(value: unknown): Record<string, unknown> | null {
    let parsed: unknown = value;
    if (typeof value === 'string') {
        const [decoded, error] = attempt<unknown>(() => JSON.parse(value));
        if (error) {
            return null;
        }
        parsed = decoded;
    }
    return typeof parsed === 'object' && parsed !== null
        ? (parsed as Record<string, unknown>)
        : null;
}

export function readPlayerMessage(
    event: MessageEvent,
    frame: HTMLIFrameElement | null,
): PlayerSnapshot | null {
    if (!YOUTUBE_ORIGINS.some((origin) => origin === event.origin)) {
        return null;
    }
    if (event.source !== frame?.contentWindow) {
        return null;
    }
    const data = messageData(event.data);
    if (data === null || data.event !== 'infoDelivery') {
        return null;
    }
    const info = messageData(data.info);
    if (info === null) {
        return null;
    }
    const state = Number(info.playerState ?? -1);
    return {
        currentTime: Number(info.currentTime ?? 0),
        duration: Number(info.duration ?? 0),
        playing: state === 1 || state === 3,
        ready: true,
    };
}

export function formatPlayerTime(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds <= 0) {
        return '0:00';
    }
    const total = Math.floor(seconds);
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const rest = total % 60;
    if (hours > 0) {
        return `${hours}:${minutes.toString().padStart(2, '0')}:${rest
            .toString()
            .padStart(2, '0')}`;
    }
    return `${minutes}:${rest.toString().padStart(2, '0')}`;
}

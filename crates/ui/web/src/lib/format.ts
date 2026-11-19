export function initials(name: string): string {
    const words = name
        .trim()
        .split(/[\s_-]+/)
        .filter((word) => word.length > 0);
    if (words.length === 0) {
        return '··';
    }
    const first = words[0]?.[0] ?? '';
    const second = words.length > 1 ? (words[1]?.[0] ?? '') : (words[0]?.[1] ?? '');
    return `${first}${second}`.toUpperCase();
}

export function shortName(user: { display_name: string; id: string; fingerprint: string }): string {
    const name = user.display_name.trim();
    return name || user.id || user.fingerprint.slice(0, 8);
}

export function clock(timestamp: number): string {
    if (timestamp <= 0) {
        return '';
    }
    return new Date(timestamp).toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
    });
}

export function dayLabel(timestamp: number): string {
    if (timestamp <= 0) {
        return 'TODAY';
    }
    const date = new Date(timestamp);
    const today = new Date();
    const yesterday = new Date();
    yesterday.setDate(today.getDate() - 1);
    const sameDay = (left: Date, right: Date): boolean =>
        left.getFullYear() === right.getFullYear() &&
        left.getMonth() === right.getMonth() &&
        left.getDate() === right.getDate();
    const stamp = date.toLocaleDateString([], { day: '2-digit', month: 'short' }).toUpperCase();
    if (sameDay(date, today)) {
        return `TODAY · ${stamp}`;
    }
    if (sameDay(date, yesterday)) {
        return `YESTERDAY · ${stamp}`;
    }
    return stamp;
}

export function dateLabel(timestamp: number): string {
    if (timestamp <= 0) {
        return 'Unknown';
    }
    return new Date(timestamp).toLocaleDateString([], {
        day: '2-digit',
        month: 'short',
        year: 'numeric',
    });
}

export function sessionClock(seconds: number): string {
    const safe = Math.max(0, Math.floor(seconds));
    const hours = Math.floor(safe / 3600);
    const minutes = Math.floor((safe % 3600) / 60);
    const remaining = safe % 60;
    return [hours, minutes, remaining].map((part) => String(part).padStart(2, '0')).join(':');
}

export function fileSize(bytes: number): string {
    if (bytes <= 0) {
        return '0 B';
    }
    const units = ['B', 'KB', 'MB', 'GB'];
    const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** exponent;
    return `${value >= 10 || exponent === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[exponent]}`;
}

export function fingerprintLabel(fingerprint: string): string {
    const normalized = fingerprint.replace(/[^a-fA-F0-9]/g, '').toUpperCase();
    if (normalized.length < 8) {
        return fingerprint;
    }
    return `${normalized.slice(0, 2)}:${normalized.slice(2, 6)}…${normalized.slice(-4)}`;
}

export function relativeSeen(timestamp: number, connected: boolean): string {
    if (connected) {
        return 'Online now';
    }
    if (timestamp <= 0) {
        return 'Offline';
    }
    const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
    if (seconds < 60) {
        return 'Seen just now';
    }
    if (seconds < 3600) {
        return `Seen ${Math.floor(seconds / 60)} min ago`;
    }
    if (seconds < 86_400) {
        return `Seen ${Math.floor(seconds / 3600)} h ago`;
    }
    return `Seen ${new Date(timestamp).toLocaleDateString([], { day: '2-digit', month: 'short' })}`;
}

export function preview(body: string, limit = 48): string {
    const normalized = body.split(/\s+/).filter(Boolean).join(' ');
    if (normalized.length <= limit) {
        return normalized;
    }
    return `${normalized.slice(0, limit - 1)}…`;
}

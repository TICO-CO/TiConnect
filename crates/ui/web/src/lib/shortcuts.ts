export interface Shortcut {
    ctrl: boolean;
    meta: boolean;
    alt: boolean;
    shift: boolean;
    key: string;
}

export function parseShortcut(value: string): Shortcut | null {
    const parts = value
        .split('+')
        .map((part) => part.trim())
        .filter((part) => part !== '');
    if (parts.length < 2) {
        return null;
    }
    const shortcut: Shortcut = {
        ctrl: false,
        meta: false,
        alt: false,
        shift: false,
        key: '',
    };
    for (const part of parts.slice(0, -1)) {
        const modifier = part.toLowerCase();
        if (modifier === 'ctrl' || modifier === 'control') {
            if (shortcut.ctrl) {
                return null;
            }
            shortcut.ctrl = true;
            continue;
        }
        if (modifier === 'shift') {
            if (shortcut.shift) {
                return null;
            }
            shortcut.shift = true;
            continue;
        }
        if (modifier === 'alt' || modifier === 'option') {
            if (shortcut.alt) {
                return null;
            }
            shortcut.alt = true;
            continue;
        }
        if (
            modifier === 'meta' ||
            modifier === 'cmd' ||
            modifier === 'command' ||
            modifier === 'super'
        ) {
            if (shortcut.meta) {
                return null;
            }
            shortcut.meta = true;
            continue;
        }
        return null;
    }
    const key = parts.at(-1) ?? '';
    if (key === '' || isModifier(key)) {
        return null;
    }
    shortcut.key = key;
    return shortcut;
}

function isModifier(value: string): boolean {
    const modifier = value.toLowerCase();
    return (
        modifier === 'ctrl' ||
        modifier === 'control' ||
        modifier === 'shift' ||
        modifier === 'alt' ||
        modifier === 'option' ||
        modifier === 'meta' ||
        modifier === 'cmd' ||
        modifier === 'command' ||
        modifier === 'super'
    );
}

export function matchesShortcut(event: KeyboardEvent, shortcut: Shortcut): boolean {
    return (
        event.ctrlKey === shortcut.ctrl &&
        event.metaKey === shortcut.meta &&
        event.altKey === shortcut.alt &&
        event.shiftKey === shortcut.shift &&
        event.key.toLowerCase() === shortcut.key.toLowerCase()
    );
}

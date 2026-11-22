import { describe, expect, it } from 'vitest';

import { matchesShortcut, parseShortcut } from './shortcuts';

function keyboardEvent(
    key: string,
    modifiers: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>> = {},
): KeyboardEvent {
    return {
        key,
        ctrlKey: false,
        metaKey: false,
        altKey: false,
        shiftKey: false,
        ...modifiers,
    } as KeyboardEvent;
}

describe('shortcuts', () => {
    it('parses modifier shortcuts', () => {
        expect(parseShortcut('Ctrl+Shift+M')).toEqual({
            ctrl: true,
            meta: false,
            alt: false,
            shift: true,
            key: 'M',
        });
    });

    it('rejects empty and modifier-only shortcuts', () => {
        expect(parseShortcut('')).toBeNull();
        expect(parseShortcut('Ctrl')).toBeNull();
        expect(parseShortcut('Ctrl+Shift')).toBeNull();
    });

    it('rejects unknown or duplicate modifiers', () => {
        expect(parseShortcut('Hyper+M')).toBeNull();
        expect(parseShortcut('Ctrl+Ctrl+M')).toBeNull();
    });

    it('matches modifiers and key exactly', () => {
        const shortcut = parseShortcut('Ctrl+Shift+M');
        expect(shortcut).not.toBeNull();
        if (shortcut === null) {
            return;
        }
        expect(
            matchesShortcut(keyboardEvent('m', { ctrlKey: true, shiftKey: true }), shortcut),
        ).toBe(true);
        expect(matchesShortcut(keyboardEvent('m', { ctrlKey: true }), shortcut)).toBe(false);
        expect(
            matchesShortcut(
                keyboardEvent('m', { ctrlKey: true, shiftKey: true, altKey: true }),
                shortcut,
            ),
        ).toBe(false);
    });
});

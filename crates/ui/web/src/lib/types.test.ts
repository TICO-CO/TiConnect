import { describe, expect, it } from 'vitest';

import { BridgeMessages } from './types';

describe('BridgeMessages', () => {
    it('recognizes events', () => {
        expect(BridgeMessages.isEvent({ type: 'ready' })).toBe(true);
    });

    it('rejects records without a string type', () => {
        expect(BridgeMessages.isEvent({ type: 1 })).toBe(false);
        expect(BridgeMessages.isEvent(null)).toBe(false);
    });

    it('recognizes replies', () => {
        expect(BridgeMessages.isReply({ id: 1, ok: true })).toBe(true);
    });

    it('rejects records without a boolean ok', () => {
        expect(BridgeMessages.isReply({ id: 1 })).toBe(false);
        expect(BridgeMessages.isReply('reply')).toBe(false);
    });

    it('does not treat events with ok as replies', () => {
        expect(BridgeMessages.isReply({ type: 'ready', ok: true })).toBe(false);
    });
});

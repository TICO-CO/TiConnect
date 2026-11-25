import { describe, expect, it } from 'vitest';

import { messageMenuAlignment, placeMessageMenu } from './message-menu';

describe('messageMenuAlignment', () => {
    it('aligns own messages to the end', () => {
        expect(messageMenuAlignment(true)).toBe('end');
    });

    it('aligns peer messages to the start', () => {
        expect(messageMenuAlignment(false)).toBe('start');
    });
});

describe('placeMessageMenu', () => {
    it('opens below the anchor with matching left edge when aligned to the start', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 600, right: 628, top: 200, bottom: 228 }, 'start'),
        ).toEqual({
            left: 600,
            top: 236,
        });
    });

    it('opens above the anchor near the viewport bottom', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 600, right: 628, top: 700, bottom: 728 }, 'start'),
        ).toEqual({
            left: 600,
            top: 580,
        });
    });

    it('clamps a start-aligned menu inside the right viewport edge', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 950, right: 978, top: 200, bottom: 228 }, 'start'),
        ).toEqual({
            left: 856,
            top: 236,
        });
    });

    it('clamps a start-aligned menu inside the left viewport edge', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 2, right: 30, top: 200, bottom: 228 }, 'start'),
        ).toEqual({
            left: 8,
            top: 236,
        });
    });

    it('opens below the anchor with matching right edge when aligned to the end', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 600, right: 628, top: 200, bottom: 228 }, 'end'),
        ).toEqual({
            left: 468,
            top: 236,
        });
    });

    it('clamps an end-aligned menu inside the right viewport edge', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 1000, right: 1028, top: 200, bottom: 228 }, 'end'),
        ).toEqual({
            left: 856,
            top: 236,
        });
    });

    it('clamps an end-aligned menu inside the left viewport edge', () => {
        expect(
            placeMessageMenu(1024, 768, { left: 2, right: 30, top: 200, bottom: 228 }, 'end'),
        ).toEqual({
            left: 8,
            top: 236,
        });
    });
});

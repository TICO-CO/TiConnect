export type MenuAlignment = 'start' | 'end';

export interface MenuAnchor {
    left: number;
    top: number;
}

export interface AnchorRect {
    left: number;
    right: number;
    top: number;
    bottom: number;
}

const MENU_WIDTH = 160;
const MENU_HEIGHT = 112;
const MENU_GAP = 8;
const VIEWPORT_MARGIN = 8;

export function messageMenuAlignment(localSender: boolean): MenuAlignment {
    return localSender ? 'end' : 'start';
}

export function placeMessageMenu(
    viewportWidth: number,
    viewportHeight: number,
    anchor: AnchorRect,
    alignment: MenuAlignment,
): MenuAnchor {
    const maxLeft = viewportWidth - MENU_WIDTH - VIEWPORT_MARGIN;
    const anchoredLeft = alignment === 'end' ? anchor.right - MENU_WIDTH : anchor.left;
    const left = Math.min(
        Math.max(anchoredLeft, VIEWPORT_MARGIN),
        Math.max(maxLeft, VIEWPORT_MARGIN),
    );
    const opensUpward = anchor.bottom + MENU_GAP + MENU_HEIGHT > viewportHeight - VIEWPORT_MARGIN;
    const rawTop = opensUpward ? anchor.top - MENU_HEIGHT - MENU_GAP : anchor.bottom + MENU_GAP;
    return { left, top: Math.max(rawTop, VIEWPORT_MARGIN) };
}

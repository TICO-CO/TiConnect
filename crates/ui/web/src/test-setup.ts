import { cleanup } from '@testing-library/svelte';
import { afterEach, vi } from 'vitest';

afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});

Object.defineProperty(URL, 'createObjectURL', {
    configurable: true,
    value: vi.fn(() => 'blob:test'),
});
Object.defineProperty(URL, 'revokeObjectURL', {
    configurable: true,
    value: vi.fn(),
});

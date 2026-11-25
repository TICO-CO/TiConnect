import { describe, expect, it } from 'vitest';

import { attachmentKind, effectiveMimeType, mimeFromName } from './attachments';

describe('mimeFromName', () => {
    it('maps audio extensions', () => {
        expect(mimeFromName('clip.mp3')).toBe('audio/mpeg');
        expect(mimeFromName('clip.OPUS')).toBe('audio/opus');
        expect(mimeFromName('voice.webm')).toBe('audio/webm');
        expect(mimeFromName('note.ogg')).toBe('audio/ogg');
        expect(mimeFromName('take.wav')).toBe('audio/wav');
        expect(mimeFromName('song.flac')).toBe('audio/flac');
        expect(mimeFromName('memo.m4a')).toBe('audio/mp4');
    });

    it('maps image extensions', () => {
        expect(mimeFromName('photo.PNG')).toBe('image/png');
        expect(mimeFromName('photo.jpg')).toBe('image/jpeg');
    });

    it('falls back to a generic mime for unknown names', () => {
        expect(mimeFromName('archive.zip')).toBe('application/octet-stream');
        expect(mimeFromName('nameless')).toBe('application/octet-stream');
        expect(mimeFromName('')).toBe('application/octet-stream');
    });
});

describe('effectiveMimeType', () => {
    it('keeps a specific mime', () => {
        expect(effectiveMimeType('audio/mpeg', 'clip.mp3')).toBe('audio/mpeg');
    });

    it('strips parameters before keeping a specific mime', () => {
        expect(effectiveMimeType('audio/webm;codecs=opus', 'voice.webm')).toBe('audio/webm');
    });

    it('replaces empty or generic mimes from the file name', () => {
        expect(effectiveMimeType('', 'clip.mp3')).toBe('audio/mpeg');
        expect(effectiveMimeType('application/octet-stream', 'clip.mp3')).toBe('audio/mpeg');
        expect(effectiveMimeType('  ', 'photo.png')).toBe('image/png');
    });
});

describe('attachmentKind', () => {
    it('detects audio from mime or extension', () => {
        expect(attachmentKind('audio/webm', 'voice.webm')).toBe('audio');
        expect(attachmentKind('', 'clip.mp3')).toBe('audio');
        expect(attachmentKind('application/octet-stream', 'clip.mp3')).toBe('audio');
    });

    it('detects images from mime or extension', () => {
        expect(attachmentKind('image/png', 'photo.png')).toBe('image');
        expect(attachmentKind('', 'photo.png')).toBe('image');
    });

    it('treats everything else as a file', () => {
        expect(attachmentKind('text/plain', 'notes.txt')).toBe('file');
        expect(attachmentKind('', 'archive.zip')).toBe('file');
    });
});

const GENERIC_MIME = 'application/octet-stream';

const AUDIO_MIME_BY_EXTENSION: Record<string, string> = {
    flac: 'audio/flac',
    m4a: 'audio/mp4',
    mp3: 'audio/mpeg',
    mp4: 'audio/mp4',
    oga: 'audio/ogg',
    ogg: 'audio/ogg',
    opus: 'audio/opus',
    wav: 'audio/wav',
    webm: 'audio/webm',
};

const IMAGE_MIME_BY_EXTENSION: Record<string, string> = {
    gif: 'image/gif',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    png: 'image/png',
    svg: 'image/svg+xml',
    webp: 'image/webp',
};

export type AttachmentKind = 'audio' | 'file' | 'image';

export function mimeFromName(name: string): string {
    const extension = name.toLowerCase().split('.').pop() ?? '';
    const audio = AUDIO_MIME_BY_EXTENSION[extension];
    if (audio !== undefined) {
        return audio;
    }
    const image = IMAGE_MIME_BY_EXTENSION[extension];
    if (image !== undefined) {
        return image;
    }
    return GENERIC_MIME;
}

export function effectiveMimeType(mime: string, name: string): string {
    const essence = mime.split(';', 1)[0]?.trim() ?? '';
    if (essence !== '' && essence !== GENERIC_MIME) {
        return essence;
    }
    return mimeFromName(name);
}

export function attachmentKind(mime: string, name: string): AttachmentKind {
    const effective = effectiveMimeType(mime, name);
    if (effective.startsWith('audio/')) {
        return 'audio';
    }
    if (effective.startsWith('image/')) {
        return 'image';
    }
    return 'file';
}

const HOSTS = ['youtube.com', 'www.youtube.com', 'm.youtube.com', 'music.youtube.com'];

export function firstVideoId(text: string): string | null {
    const tokens = text.split(/\s+/);
    for (const token of tokens) {
        const videoId = parseVideoId(token.replace(/[),.;:'"\]}]+$/g, ''));
        if (videoId !== null) {
            return videoId;
        }
    }
    return null;
}

export function parseVideoId(value: string): string | null {
    const withoutScheme = value.replace(/^https?:\/\//, '');
    const short = withoutScheme.match(/^youtu\.be\/([A-Za-z0-9_-]{1,32})/);
    if (short !== null) {
        return short[1] ?? null;
    }
    const noCookie = withoutScheme.match(
        /^www\.youtube-nocookie\.com\/(?:embed|shorts|live|v)\/([A-Za-z0-9_-]{1,32})/,
    );
    if (noCookie !== null) {
        return noCookie[1] ?? null;
    }
    for (const host of HOSTS) {
        if (!withoutScheme.startsWith(host)) {
            continue;
        }
        const rest = withoutScheme.slice(host.length).replace(/^\//, '');
        const watch = rest.match(/^watch[^#]*[?&]v=([A-Za-z0-9_-]{1,32})/);
        if (watch !== null) {
            return watch[1] ?? null;
        }
        const path = rest.match(/^(?:shorts|embed|live|v)\/([A-Za-z0-9_-]{1,32})/);
        if (path !== null) {
            return path[1] ?? null;
        }
    }
    return null;
}

<script lang="ts">
    import type { AppState } from '../lib';
    import { clock, dayLabel, fileSize, fingerprintLabel, sessionClock, shortName } from '../lib';
    import { firstVideoId } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let stream: HTMLDivElement | null = $state(null);
    let fileInput: HTMLInputElement | null = $state(null);
    let composer: HTMLTextAreaElement | null = $state(null);

    const selectedUser = $derived(app.userById(app.selectedUserId));
    const currentParty = $derived(
        app.party !== null && app.party.user_id === app.selectedUserId ? app.party : null,
    );
    const announcementVideo = $derived(
        app.player?.videoId ?? currentParty?.video_id ?? latestVideoId(),
    );
    const callLogs = $derived(
        app.callLogs.filter((log) => log.userId === app.selectedUserId).slice(-3),
    );

    function latestVideoId(): string {
        for (let index = app.messages.length - 1; index >= 0; index -= 1) {
            const message = app.messages[index];
            if (message?.attachment_hash) {
                continue;
            }
            const videoId = firstVideoId(message?.body ?? '');
            if (videoId !== null) {
                return videoId;
            }
        }
        return '';
    }

    function isOnlyVideoLink(body: string): boolean {
        const trimmed = body.trim();
        return firstVideoId(trimmed) !== null && !trimmed.split(/\s+/).slice(1).length;
    }

    function videoTitle(videoId: string): string {
        if (app.demo) {
            return 'How we built TiConnect for the LAN';
        }
        return `YouTube video ${videoId}`;
    }

    function send(event: SubmitEvent) {
        event.preventDefault();
        void app.sendMessage();
    }

    function onComposerKey(event: KeyboardEvent) {
        if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
            return;
        }
        event.preventDefault();
        void app.sendMessage();
    }

    function attach(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (file) {
            void app.sendFile(file);
        }
        input.value = '';
    }

    function playTogether() {
        if (latestVideoId() === '') {
            app.notice = 'Paste a YouTube link in the conversation first.';
            composer?.focus();
            return;
        }
        app.openPlayer(latestVideoId());
        if (selectedUser !== null) {
            void app.startParty(latestVideoId(), selectedUser.id);
        }
    }

    $effect(() => {
        if (app.messages.length > 0 && stream !== null) {
            stream.scrollTop = stream.scrollHeight;
        }
    });
</script>

<section class="chat-pane">
    {#if selectedUser}
        <header class="chat-header">
            <button
                class="identity"
                onclick={() => app.navigate({ kind: 'profile', id: selectedUser.id })}
            >
                <Avatar user={selectedUser} size={44} />
                <span class="identity-copy">
                    <span class="display name">{shortName(selectedUser)}</span>
                    <span class="presence">
                        <span class="dot" class:online={selectedUser.connected}></span>
                        {selectedUser.connected ? 'Online' : 'Offline'}
                    </span>
                </span>
            </button>
            <div class="actions">
                <button class="btn btn-small" onclick={() => void app.startCall(selectedUser.id)}>
                    Call
                    <Icon name="call" size={15} />
                </button>
                <button
                    class="btn btn-small"
                    onclick={() => void app.requestRemote(selectedUser.id)}
                >
                    Remote desktop
                    <Icon name="remote" size={15} />
                </button>
                <button class="btn btn-small" onclick={playTogether}>
                    Watch party
                    <Icon name="chevron-right" size={13} />
                </button>
            </div>
        </header>
        {#if announcementVideo}
            <div class="youtube-bar">
                <span class="lamp"></span>
                <span class="youtube-label">YouTube</span>
                <span class="youtube-title">{videoTitle(announcementVideo)} · plays in overlay</span
                >
                <button
                    class="icon-button small"
                    onclick={() => app.closePlayer()}
                    aria-label="Dismiss video notice"
                >
                    <Icon name="close" size={14} />
                </button>
            </div>
        {/if}
        <div class="stream scroll" bind:this={stream}>
            {#each app.messages as message, index (message.id)}
                {@const previous = app.messages[index - 1]}
                {@const showDay =
                    previous === undefined ||
                    dayLabel(previous.sent_at) !== dayLabel(message.sent_at)}
                {#if showDay}
                    <div class="day"><span>{dayLabel(message.sent_at)}</span></div>
                {/if}
                {#if message.body === '' && message.attachment_hash === ''}
                    <p class="empty">Message unavailable.</p>
                {:else}
                    <article class="message" class:local={message.local_sender}>
                        {#if message.body && !isOnlyVideoLink(message.body)}
                            <p class="body">{message.body}</p>
                        {/if}
                        {#if message.attachment_hash}
                            <button
                                class="attachment"
                                onclick={() => void app.openAttachment(message.attachment_hash)}
                            >
                                <span class="file-icon"
                                    ><Icon name="file" size={18} color="var(--ink-2)" /></span
                                >
                                <span class="file-copy">
                                    <strong>{message.attachment_name || 'Attachment'}</strong>
                                    <span
                                        >BLAKE3 {fingerprintLabel(message.attachment_hash)} · {fileSize(
                                            message.attachment_size,
                                        )}</span
                                    >
                                </span>
                            </button>
                        {/if}
                        {#if firstVideoId(message.body)}
                            {@const videoId = firstVideoId(message.body) ?? ''}
                            <div class="video-card">
                                <div class="thumb">
                                    <img
                                        src={`https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`}
                                        alt=""
                                    />
                                    <span class="thumb-play"
                                        ><Icon name="play" size={18} color="var(--base)" /></span
                                    >
                                </div>
                                <div class="video-copy">
                                    <strong>{videoTitle(videoId)}</strong>
                                    <span>TiConnect Labs · 18:42</span>
                                    <span class="playing-label"
                                        >Playing in overlay · 2 in queue</span
                                    >
                                    <button
                                        class="btn btn-small"
                                        onclick={() =>
                                            void app.startParty(videoId, selectedUser.id)}
                                    >
                                        Watch party
                                        <Icon name="chevron-right" size={12} />
                                    </button>
                                </div>
                            </div>
                        {/if}
                        <div class="meta mono-data">
                            <span>{clock(message.sent_at || message.received_at)}</span>
                            {#if message.local_sender}
                                {#if message.state === 'read'}
                                    <span class="receipt read"
                                        ><Icon
                                            name="checks"
                                            size={14}
                                            color="var(--accent)"
                                        /></span
                                    >
                                {:else if message.state === 'delivered'}
                                    <span class="receipt"
                                        ><Icon name="checks" size={14} color="var(--ink-3)" /></span
                                    >
                                {:else if message.state === 'failed'}
                                    <span class="receipt error"
                                        ><Icon name="alert" size={13} color="var(--error)" /></span
                                    >
                                {:else}
                                    <span class="receipt"
                                        ><Icon name="check" size={13} color="var(--ink-3)" /></span
                                    >
                                {/if}
                            {/if}
                        </div>
                    </article>
                {/if}
            {/each}
            {#if callLogs.length > 0}
                {#each callLogs as log (log.id)}
                    <div class="call-line">
                        <span class="rule"></span>
                        <span class="call-label">
                            <Icon name="call" size={12} color="var(--ink-3)" />
                            {log.reason === 'missed' ? 'Missed call' : 'Call ended'} · {sessionClock(
                                log.durationSeconds,
                            )}
                        </span>
                        <span class="rule"></span>
                    </div>
                {/each}
            {/if}
            {#if app.typing[selectedUser.id]}
                <p class="typing">{shortName(selectedUser)} is typing…</p>
            {/if}
            {#if app.messages.length === 0}
                <div class="empty-conversation">
                    <p class="display">A quiet line is still open.</p>
                    <p class="empty">Say hello — messages stay on this network.</p>
                </div>
            {/if}
        </div>
        <form class="composer" onsubmit={send}>
            <label class="sr-only" for="message-body">Message</label>
            <textarea
                id="message-body"
                bind:this={composer}
                value={app.draft}
                oninput={(event) =>
                    app.updateDraft((event.currentTarget as HTMLTextAreaElement).value)}
                onkeydown={onComposerKey}
                placeholder="Write a message"
                rows="1"></textarea>
            <div class="composer-row">
                <input class="sr-only" type="file" bind:this={fileInput} onchange={attach} />
                <button
                    type="button"
                    class="icon-button small"
                    onclick={() => fileInput?.click()}
                    aria-label="Attach file"
                >
                    <Icon name="paperclip" size={16} color="var(--ink-2)" />
                </button>
                <button
                    type="button"
                    class="icon-button small"
                    onclick={() =>
                        (app.notice =
                            'Voice recording is not wired yet. Attach an audio file instead.')}
                    aria-label="Record voice message"
                >
                    <Icon name="mic" size={16} color="var(--ink-2)" />
                </button>
                <span class="send-hint">Enter to send</span>
                <button type="submit" class="send-button" aria-label="Send message">
                    <Icon name="send" size={18} color="var(--base)" />
                </button>
            </div>
        </form>
    {:else}
        <div class="no-chat">
            <p class="display">Pick a conversation.</p>
            <p class="empty">The rail lists everyone discovered on this LAN.</p>
        </div>
    {/if}
</section>

<style>
    .chat-pane {
        display: flex;
        flex-direction: column;
        min-width: 0;
        height: 100%;
        flex: 1;
        background: var(--base);
    }

    .chat-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        height: 88px;
        flex-shrink: 0;
        padding-inline: 28px;
        gap: 14px;
        background: var(--rail);
    }

    .identity {
        display: flex;
        align-items: center;
        gap: 14px;
        min-width: 0;
        padding: 0;
        border: 0;
        background: transparent;
        color: var(--ink);
        text-align: left;
    }

    .identity-copy {
        display: flex;
        flex-direction: column;
        min-width: 0;
        gap: 3px;
    }

    .name {
        overflow: hidden;
        font-size: var(--step-2);
        line-height: 26px;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .presence {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--ink-3);
        font-size: 13px;
        line-height: 16px;
    }

    .dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        border: 1px solid var(--line-strong);
    }

    .dot.online {
        border-color: var(--success);
        background: var(--success);
    }

    .actions {
        display: flex;
        gap: 10px;
        flex-shrink: 0;
    }

    .youtube-bar {
        display: flex;
        align-items: center;
        height: 36px;
        flex-shrink: 0;
        padding-inline: 28px;
        gap: 8px;
        background: var(--surface-2);
        border-bottom: 1px solid var(--line);
        color: var(--ink-2);
        font-size: 12px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }

    .lamp {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--warning);
    }

    .youtube-label {
        color: var(--ink-3);
        font-weight: 700;
    }

    .youtube-title {
        overflow: hidden;
        flex: 1;
        color: var(--ink-2);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .stream {
        display: flex;
        flex-direction: column;
        min-height: 0;
        flex: 1;
        padding: 12px 28px;
        overflow-y: auto;
        gap: 4px;
    }

    .day {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 8px 0 14px;
    }

    .day span {
        padding: 4px 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-pill);
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.06em;
    }

    .message {
        display: flex;
        flex-direction: column;
        align-self: flex-start;
        max-width: min(760px, 78%);
        gap: 6px;
        padding-bottom: 12px;
        color: var(--ink);
    }

    .message.local {
        align-self: flex-end;
    }

    .body {
        margin: 0;
        font-size: var(--step-0);
        line-height: 155%;
        overflow-wrap: anywhere;
        white-space: pre-wrap;
    }

    .meta {
        display: flex;
        align-items: center;
        justify-content: flex-start;
        gap: 5px;
        color: var(--ink-3);
        font-size: 12px;
        line-height: 16px;
    }

    .message.local .meta {
        justify-content: flex-end;
        color: var(--ink-3);
    }

    .receipt {
        display: inline-flex;
    }

    .receipt.read {
        color: var(--accent);
    }

    .attachment {
        display: flex;
        align-items: center;
        gap: 12px;
        width: 100%;
        padding: 12px 14px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
        color: var(--ink);
        text-align: left;
    }

    .attachment:hover {
        border-color: var(--line-strong);
        background: var(--surface-2);
    }

    .file-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 32px;
        height: 32px;
        flex-shrink: 0;
        border-radius: var(--radius-s);
        background: var(--base);
    }

    .file-copy {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .file-copy strong {
        font-size: 14px;
    }

    .file-copy span {
        color: var(--ink-3);
        font-size: 12px;
    }

    .video-card {
        display: flex;
        gap: 16px;
        width: 100%;
        padding: 12px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .thumb {
        position: relative;
        width: 166px;
        height: 94px;
        flex-shrink: 0;
        overflow: hidden;
        border-radius: var(--radius-s);
        background: var(--surface-2);
    }

    .thumb img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        opacity: 0.82;
    }

    .thumb-play {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .video-copy {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: 4px;
        min-width: 0;
    }

    .video-copy strong {
        font-size: 15px;
        line-height: 20px;
    }

    .video-copy span {
        color: var(--ink-3);
        font-size: 12px;
    }

    .video-copy .playing-label {
        color: var(--ink-2);
        letter-spacing: 0.05em;
        text-transform: uppercase;
    }

    .video-copy .btn {
        margin-top: 4px;
    }

    .call-line {
        display: flex;
        align-items: center;
        justify-content: center;
        width: min(720px, 100%);
        align-self: center;
        padding: 8px 0;
        gap: 12px;
    }

    .rule {
        height: 1px;
        flex: 1;
        background: var(--line);
    }

    .call-label {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--ink-3);
        font-size: 12px;
        letter-spacing: 0.04em;
    }

    .typing {
        margin: 0;
        color: var(--ink-3);
        font-size: 12px;
    }

    .empty-conversation,
    .no-chat {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        min-height: 100%;
        gap: 8px;
        text-align: center;
    }

    .empty-conversation .display,
    .no-chat .display {
        margin: 0;
        font-size: var(--step-3);
        color: var(--ink);
    }

    .composer {
        display: flex;
        flex-direction: column;
        flex-shrink: 0;
        margin: 0 20px 20px;
        padding: 14px 16px 10px;
        border: 1px solid var(--line);
        border-radius: var(--radius-m);
        background: var(--surface);
    }

    .composer textarea {
        width: 100%;
        max-height: 120px;
        min-height: 36px;
        resize: none;
        border: 0;
        outline: 0;
        background: transparent;
        color: var(--ink);
        font-size: var(--step-0);
        line-height: 1.45;
    }

    .composer textarea::placeholder {
        color: var(--ink-3);
    }

    .composer-row {
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .send-hint {
        margin-left: auto;
        color: var(--ink-3);
        font-size: 11px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .send-button {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 40px;
        height: 32px;
        flex-shrink: 0;
        border: 1px solid var(--accent);
        border-radius: var(--radius-pill);
        background: var(--accent);
    }
</style>

<script lang="ts">
    import { tick } from 'svelte';

    import type { AppState, CallLog, Message } from '../lib';
    import {
        attachmentKind,
        clock,
        dayLabel,
        fileSize,
        fingerprintLabel,
        sessionClock,
        shortName,
    } from '../lib';
    import { firstVideoId } from '../lib';
    import Avatar from './Avatar.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import EditMessageDialog from './EditMessageDialog.svelte';
    import Icon from './Icon.svelte';
    import SkeletonBlock from './SkeletonBlock.svelte';
    import StateBanner from './StateBanner.svelte';
    import VoiceMessage from './VoiceMessage.svelte';

    interface Props {
        app: AppState;
    }

    type TimelineEntry =
        | { kind: 'message'; at: number; id: string; message: Message }
        | { kind: 'call'; at: number; id: string; log: CallLog };

    let { app }: Props = $props();
    let stream: HTMLDivElement | null = $state(null);
    let fileInput: HTMLInputElement | null = $state(null);
    let composer: HTMLTextAreaElement | null = $state(null);
    let recordingTrack: HTMLElement | null = $state(null);
    let recordingTrackWidth = $state(0);
    let imageUrls = $state<Record<string, string>>({});
    let imageLoading = $state<Record<string, boolean>>({});
    let imageErrors = $state<Record<string, string>>({});
    let preview = $state<{ url: string; name: string } | null>(null);
    let editingMessage = $state<Message | null>(null);
    let editingBody = $state('');
    let editError = $state('');
    let deleteTarget = $state<Message | null>(null);
    let deleteError = $state('');
    let prependingOlderMessages = false;
    const loadingTimelineRows = [0, 1, 2, 3, 4];
    const recordingBarCount = $derived(
        recordingTrackWidth > 0 ? Math.max(1, Math.floor(recordingTrackWidth / 6)) : 24,
    );
    const recordingLevels = $derived(app.recordingWaveform.slice(-recordingBarCount));

    const selectedUser = $derived(app.userById(app.selectedUserId));
    const draftBytes = $derived(new TextEncoder().encode(app.draft).length);
    const overLimit = $derived(draftBytes > 32 * 1024);
    const composerBlocked = $derived(
        app.connection !== 'online'
            ? 'Daemon offline — messages cannot be sent.'
            : selectedUser?.policy === 'ignore'
              ? 'This user is blocked. Unblock them from their profile to send messages.'
              : '',
    );
    const currentParty = $derived(
        app.party !== null && app.party.participants.includes(app.selectedUserId)
            ? app.party
            : null,
    );
    const announcementVideo = $derived(
        app.player?.videoId ?? currentParty?.video_id ?? latestVideoId(),
    );
    const timeline = $derived.by((): TimelineEntry[] => {
        const oldestLoaded = app.messages.reduce<number | null>((earliest, message) => {
            const at = messageTime(message);
            return at > 0 && (earliest === null || at < earliest) ? at : earliest;
        }, null);
        const logs = app.callLogs.filter(
            (log) =>
                log.user_id === app.selectedUserId &&
                (oldestLoaded === null || logTime(log) >= oldestLoaded),
        );
        const entries: TimelineEntry[] = [
            ...app.messages.map((message) => ({
                kind: 'message' as const,
                at: messageTime(message),
                id: `m:${message.id}`,
                message,
            })),
            ...logs.map((log) => ({
                kind: 'call' as const,
                at: logTime(log),
                id: `c:${log.id}`,
                log,
            })),
        ];
        entries.sort((left, right) => {
            if (left.at !== right.at) {
                return left.at - right.at;
            }
            if (left.kind === right.kind) {
                return 0;
            }
            return left.kind === 'message' ? -1 : 1;
        });
        return entries;
    });

    function messageTime(message: Message): number {
        return message.sent_at || message.received_at;
    }

    function logTime(log: CallLog): number {
        return log.ended_at || log.started_at;
    }

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

    function canSend(): boolean {
        return (
            composerBlocked === '' &&
            !overLimit &&
            app.draft.trim() !== '' &&
            app.connection === 'online'
        );
    }

    function send(event: SubmitEvent) {
        event.preventDefault();
        if (!canSend()) {
            return;
        }
        void app.sendMessage();
    }

    function onComposerKey(event: KeyboardEvent) {
        if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
            return;
        }
        event.preventDefault();
        if (canSend()) {
            void app.sendMessage();
        }
    }

    function attach(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (file) {
            void app.sendFile(file);
        }
        input.value = '';
    }

    function openEdit(message: Message): void {
        editingMessage = message;
        editingBody = message.body;
        editError = '';
    }

    async function saveEdit(body: string): Promise<void> {
        if (editingMessage === null) {
            return;
        }
        const saved = await app.editMessage(editingMessage.id, body);
        if (!saved) {
            editError = 'Message could not be saved.';
            return;
        }
        editingMessage = null;
        editingBody = '';
        editError = '';
    }

    function closeEdit(): void {
        if (app.messageActionPending !== '') {
            return;
        }
        editingMessage = null;
        editError = '';
    }

    function closeDelete(): void {
        if (app.messageActionPending !== '') {
            return;
        }
        deleteTarget = null;
        deleteError = '';
    }

    async function confirmDelete(): Promise<void> {
        if (deleteTarget === null) {
            return;
        }
        const deleted = await app.deleteMessage(deleteTarget.id);
        if (!deleted) {
            deleteError = 'Message could not be deleted.';
            return;
        }
        deleteTarget = null;
        deleteError = '';
    }

    async function loadImage(messageId: string, hash: string): Promise<void> {
        if (imageUrls[messageId] !== undefined || imageLoading[messageId] === true) {
            return;
        }
        const errors = { ...imageErrors };
        delete errors[messageId];
        imageErrors = errors;
        imageLoading = { ...imageLoading, [messageId]: true };
        const url = await app.attachmentUrl(hash);
        imageLoading = { ...imageLoading, [messageId]: false };
        if (url !== '') {
            imageUrls = { ...imageUrls, [messageId]: url };
        } else {
            imageErrors = { ...imageErrors, [messageId]: 'Image unavailable' };
        }
    }

    function showImage(messageId: string, hash: string, name: string): void {
        const url = imageUrls[messageId];
        if (url !== undefined) {
            preview = { url, name };
            return;
        }
        void loadImage(messageId, hash).then(() => {
            const loaded = imageUrls[messageId];
            if (loaded !== undefined) {
                preview = { url: loaded, name };
            }
        });
    }

    function transferLabel(hash: string): string {
        const transfer = app.transferFor(hash);
        if (transfer === null || transfer.done) {
            return '';
        }
        if (transfer.total > 0) {
            const percent = Math.min(100, Math.round((transfer.bytes / transfer.total) * 100));
            return `Transferring · ${percent}%`;
        }
        return 'Transferring…';
    }

    function imageStatus(messageId: string, hash: string): string {
        const transfer = app.transferFor(hash);
        if (transfer?.failed) {
            return 'Image transfer failed. Tap to retry.';
        }
        const transferText = transferLabel(hash);
        if (transferText !== '') {
            return transferText;
        }
        return imageErrors[messageId] ? 'Image unavailable. Tap to retry.' : 'Loading image…';
    }

    $effect(() => {
        for (const message of app.messages) {
            if (attachmentKind(message.attachment_mime, message.attachment_name) !== 'image') {
                continue;
            }
            if (imageErrors[message.id] !== undefined) {
                continue;
            }
            const transfer = app.transferFor(message.attachment_hash);
            if (transfer !== null && (!transfer.done || transfer.failed !== '')) {
                continue;
            }
            void loadImage(message.id, message.attachment_hash);
        }
    });

    $effect(() => {
        if (recordingTrack === null || typeof ResizeObserver === 'undefined') {
            return;
        }
        const observer = new ResizeObserver((entries) => {
            recordingTrackWidth = entries[0]?.contentRect.width ?? 0;
        });
        observer.observe(recordingTrack);
        return () => observer.disconnect();
    });

    async function loadOlderMessages(): Promise<void> {
        if (stream === null || stream.scrollTop > 48) {
            return;
        }
        if (!app.hasOlderMessages || app.loadingOlderMessages) {
            return;
        }
        const previousHeight = stream.scrollHeight;
        const previousTop = stream.scrollTop;
        prependingOlderMessages = true;
        await app.loadOlderMessages();
        await tick();
        if (stream !== null) {
            stream.scrollTop = stream.scrollHeight - previousHeight + previousTop;
        }
        prependingOlderMessages = false;
    }

    function handleStreamScroll(): void {
        void loadOlderMessages();
    }

    $effect(() => {
        if (timeline.length > 0 && stream !== null && !prependingOlderMessages) {
            stream.scrollTop = stream.scrollHeight;
        }
    });
</script>

<section class="flex h-full min-w-0 flex-1 flex-col bg-base">
    {#if selectedUser}
        <header class="flex h-[88px] shrink-0 items-center justify-between gap-3.5 bg-rail px-7">
            <button
                class="flex min-w-0 items-center gap-3.5 border-0 bg-transparent p-0 text-left text-ink"
                onclick={() => app.navigate({ kind: 'profile', id: selectedUser.id })}
            >
                <Avatar
                    user={selectedUser}
                    image={app.avatarUrl(selectedUser.fingerprint)}
                    size={44}
                />
                <span class="flex min-w-0 flex-col gap-[3px]">
                    <span
                        class="truncate font-display text-step-2 leading-[26px] font-normal tracking-[-0.01em]"
                        >{shortName(selectedUser)}</span
                    >
                    <span class="flex items-center gap-1.5 text-[13px] leading-4 text-ink-3">
                        <span
                            class="size-1.5 rounded-full border border-line-strong data-online:border-success data-online:bg-success"
                            data-online={selectedUser.connected ? '' : undefined}
                        ></span>
                        {selectedUser.connected ? 'Online' : 'Offline'}
                    </span>
                </span>
            </button>
            <div class="flex shrink-0 gap-2">
                <div class="group relative">
                    <button
                        class="inline-flex size-10 shrink-0 items-center justify-center rounded-pill border border-line bg-transparent text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                        onclick={() => void app.startCall(selectedUser.id)}
                        aria-label="Call"
                    >
                        <Icon name="call" size={17} />
                    </button>
                    <span
                        class="pointer-events-none absolute top-full left-1/2 z-50 mt-2 -translate-x-1/2 rounded-sm border border-line bg-surface-2 px-2 py-1 text-[11px] leading-4 font-bold whitespace-nowrap text-ink opacity-0 shadow-soft transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100"
                        role="tooltip">Call</span
                    >
                </div>
                <div class="group relative">
                    <button
                        class="inline-flex size-10 shrink-0 items-center justify-center rounded-pill border border-line bg-transparent text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                        onclick={() => void app.requestRemote(selectedUser.id)}
                        aria-label="Remote desktop"
                    >
                        <Icon name="remote" size={17} />
                    </button>
                    <span
                        class="pointer-events-none absolute top-full left-1/2 z-50 mt-2 -translate-x-1/2 rounded-sm border border-line bg-surface-2 px-2 py-1 text-[11px] leading-4 font-bold whitespace-nowrap text-ink opacity-0 shadow-soft transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100"
                        role="tooltip">Remote desktop</span
                    >
                </div>
                <div class="group relative">
                    <button
                        class="inline-flex size-10 shrink-0 items-center justify-center rounded-pill border border-line bg-transparent text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                        onclick={() => app.openWatchParty([selectedUser.id])}
                        aria-label="Watch party"
                    >
                        <Icon name="party" size={17} />
                    </button>
                    <span
                        class="pointer-events-none absolute top-full left-1/2 z-50 mt-2 -translate-x-1/2 rounded-sm border border-line bg-surface-2 px-2 py-1 text-[11px] leading-4 font-bold whitespace-nowrap text-ink opacity-0 shadow-soft transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100"
                        role="tooltip">Watch party</span
                    >
                </div>
            </div>
        </header>
        {#if announcementVideo}
            <div
                class="flex h-9 shrink-0 items-center gap-2 border-b border-line bg-surface-2 px-7 text-xs tracking-[0.06em] text-ink-2 uppercase"
            >
                <span class="size-1.5 rounded-full bg-warning"></span>
                <span class="font-bold text-ink-3">YouTube</span>
                <span class="min-w-0 flex-1 truncate text-ink-2"
                    >{videoTitle(announcementVideo)} · plays in overlay</span
                >
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => app.closePlayer()}
                    aria-label="Dismiss video notice"
                >
                    <Icon name="close" size={14} />
                </button>
            </div>
        {/if}
        <div
            class="flex scroll min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-7 py-3"
            bind:this={stream}
            onscroll={handleStreamScroll}
        >
            {#if app.messagesLoad === 'loading'}
                <div
                    class="flex min-h-full flex-col justify-end gap-4 py-4"
                    role="status"
                    aria-label="Loading messages"
                >
                    {#each loadingTimelineRows as rowIndex (rowIndex)}
                        <div
                            class="flex max-w-[min(560px,70%)] flex-col gap-2 {rowIndex % 2 === 0
                                ? 'self-start'
                                : 'self-end'}"
                        >
                            <SkeletonBlock width={rowIndex % 3 === 0 ? '82%' : '64%'} height={20} />
                            <SkeletonBlock width="42%" height={12} />
                        </div>
                    {/each}
                </div>
            {:else if app.messagesLoad === 'error'}
                <div class="flex min-h-full items-center justify-center px-4 py-8">
                    <div class="w-full max-w-[420px]">
                        <StateBanner
                            tone="error"
                            title="Messages did not load"
                            body={app.messagesError ||
                                'The daemon did not return this conversation.'}
                            actionLabel="Retry"
                            onaction={() => void app.loadMessages(app.selectedUserId)}
                        />
                    </div>
                </div>
            {:else}
                {#if app.loadingOlderMessages}
                    <div
                        class="flex shrink-0 justify-center py-2 text-step-xs text-ink-3"
                        role="status"
                    >
                        Loading older messages…
                    </div>
                {/if}
                {#if app.olderMessagesError}
                    <div class="shrink-0 pb-2">
                        <StateBanner
                            tone="error"
                            title="Older messages did not load"
                            body={app.olderMessagesError}
                            actionLabel="Retry"
                            onaction={() => void app.loadOlderMessages()}
                        />
                    </div>
                {/if}
                {#each timeline as entry, index (entry.id)}
                    {@const previous = timeline[index - 1]}
                    {@const showDay =
                        previous === undefined || dayLabel(previous.at) !== dayLabel(entry.at)}
                    {#if showDay}
                        <div class="flex items-center justify-center pt-2 pb-3.5">
                            <span
                                class="rounded-pill border border-line px-3 py-1 text-xs tracking-[0.06em] text-ink-3"
                                >{dayLabel(entry.at)}</span
                            >
                        </div>
                    {/if}
                    {#if entry.kind === 'call'}
                        <div
                            class="flex w-[min(720px,100%)] items-center justify-center gap-3 self-center py-2"
                        >
                            <span class="h-px flex-1 bg-line"></span>
                            <span
                                class="flex items-center gap-1.5 text-xs tracking-[0.04em] text-ink-3"
                            >
                                <Icon name="call" size={12} color="var(--color-ink-3)" />
                                {entry.log.reason === 'timeout' ? 'Missed call' : 'Call ended'} · {sessionClock(
                                    entry.log.duration_secs,
                                )} · {clock(entry.log.ended_at)}
                            </span>
                            <span class="h-px flex-1 bg-line"></span>
                        </div>
                    {/if}
                    {#if entry.kind === 'message'}
                        {@const message = entry.message}
                        {#if message.deleted_at === 0 && message.body === '' && message.attachment_hash === ''}
                            <p class="m-0 text-step-xs text-ink-3">Message unavailable.</p>
                        {:else}
                            <article
                                class="group flex max-w-[min(760px,78%)] flex-col gap-1.5 self-start pb-3 text-ink data-local:self-end"
                                class:w-full={attachmentKind(
                                    message.attachment_mime,
                                    message.attachment_name,
                                ) === 'audio'}
                                data-local={message.local_sender ? '' : undefined}
                            >
                                {#if message.deleted_at > 0}
                                    <p class="m-0 text-step-xs leading-[1.5] text-ink-3 italic">
                                        Message deleted
                                    </p>
                                {:else}
                                    {#if message.body && !isOnlyVideoLink(message.body)}
                                        <p
                                            class="m-0 text-step-0 leading-[155%] [overflow-wrap:anywhere] whitespace-pre-wrap"
                                        >
                                            {message.body}
                                        </p>
                                    {/if}
                                    {#if message.attachment_hash}
                                        {#if attachmentKind(message.attachment_mime, message.attachment_name) === 'image'}
                                            <button
                                                class="flex w-full flex-col overflow-hidden rounded-md border border-line bg-surface p-1 text-left hover:border-line-strong"
                                                onclick={() =>
                                                    showImage(
                                                        message.id,
                                                        message.attachment_hash,
                                                        message.attachment_name || 'Image',
                                                    )}
                                            >
                                                {#if imageUrls[message.id] !== undefined}
                                                    <img
                                                        class="max-h-[360px] w-full rounded-sm object-contain"
                                                        src={imageUrls[message.id]}
                                                        alt={message.attachment_name ||
                                                            'Image attachment'}
                                                    />
                                                {:else}
                                                    <div
                                                        class="flex min-h-[120px] items-center justify-center rounded-sm bg-surface-2 px-4 text-center text-step-xs text-ink-3"
                                                    >
                                                        {imageStatus(
                                                            message.id,
                                                            message.attachment_hash,
                                                        )}
                                                    </div>
                                                {/if}
                                                <span class="px-2 py-1.5 text-xs text-ink-3"
                                                    >{message.attachment_name || 'Image'} · {fileSize(
                                                        message.attachment_size,
                                                    )}{transferLabel(message.attachment_hash)
                                                        ? ` · ${transferLabel(message.attachment_hash)}`
                                                        : ''}</span
                                                >
                                            </button>
                                        {:else if attachmentKind(message.attachment_mime, message.attachment_name) === 'audio'}
                                            <VoiceMessage {app} {message} />
                                        {:else}
                                            <button
                                                class="flex w-full items-center gap-3 rounded-md border border-line bg-surface px-3.5 py-3 text-left text-ink hover:border-line-strong hover:bg-surface-2"
                                                onclick={() =>
                                                    void app.saveAttachment(
                                                        message.attachment_hash,
                                                        message.attachment_name || 'attachment',
                                                    )}
                                            >
                                                <span
                                                    class="flex size-8 shrink-0 items-center justify-center rounded-sm bg-base"
                                                    ><Icon
                                                        name="file"
                                                        size={18}
                                                        color="var(--color-ink-2)"
                                                    /></span
                                                >
                                                <span class="flex flex-col gap-0.5">
                                                    <strong class="text-[14px]"
                                                        >{message.attachment_name ||
                                                            'Attachment'}</strong
                                                    >
                                                    <span class="text-xs text-ink-3"
                                                        >BLAKE3 {fingerprintLabel(
                                                            message.attachment_hash,
                                                        )} ·
                                                        {fileSize(
                                                            message.attachment_size,
                                                        )}{transferLabel(message.attachment_hash)
                                                            ? ` · ${transferLabel(message.attachment_hash)}`
                                                            : ''}</span
                                                    >
                                                </span>
                                            </button>
                                        {/if}
                                    {/if}
                                    {#if firstVideoId(message.body)}
                                        {@const videoId = firstVideoId(message.body) ?? ''}
                                        <div
                                            class="flex w-full gap-4 rounded-md border border-line bg-surface p-3"
                                        >
                                            <div
                                                class="relative h-[94px] w-[166px] shrink-0 overflow-hidden rounded-sm bg-surface-2"
                                            >
                                                <img
                                                    class="h-full w-full object-cover opacity-[0.82]"
                                                    src={`https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`}
                                                    alt=""
                                                />
                                                <span
                                                    class="absolute inset-0 flex items-center justify-center"
                                                    ><Icon
                                                        name="play"
                                                        size={18}
                                                        color="var(--color-base)"
                                                    /></span
                                                >
                                            </div>
                                            <div class="flex min-w-0 flex-col items-start gap-1">
                                                <strong class="text-[15px] leading-5"
                                                    >{videoTitle(videoId)}</strong
                                                >
                                                <span class="text-xs text-ink-3"
                                                    >TiConnect Labs · 18:42</span
                                                >
                                                <span
                                                    class="text-xs tracking-[0.05em] text-ink-2 uppercase"
                                                    >Playing in overlay · 2 in queue</span
                                                >
                                                <button
                                                    class="mt-1 inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                                                    onclick={() =>
                                                        void app.startParty(videoId, [
                                                            selectedUser.id,
                                                        ])}
                                                >
                                                    Watch party
                                                    <Icon name="chevron-right" size={12} />
                                                </button>
                                            </div>
                                        </div>
                                    {/if}
                                {/if}
                                <div
                                    class="flex items-center justify-start gap-[5px] text-xs leading-4 text-ink-3 tabular-nums group-data-local:justify-end"
                                >
                                    <span>{clock(message.sent_at || message.received_at)}</span>
                                    {#if message.edited_at > 0}
                                        <span>· edited</span>
                                    {/if}
                                    {#if message.local_sender}
                                        {#if message.state === 'read'}
                                            <span class="inline-flex text-accent"
                                                ><Icon
                                                    name="checks"
                                                    size={14}
                                                    color="var(--color-accent)"
                                                /></span
                                            >
                                        {:else if message.state === 'delivered'}
                                            <span class="inline-flex"
                                                ><Icon
                                                    name="checks"
                                                    size={14}
                                                    color="var(--color-ink-3)"
                                                /></span
                                            >
                                        {:else if message.state === 'failed' || message.state === 'expired'}
                                            <span class="inline-flex text-error"
                                                ><Icon
                                                    name="alert"
                                                    size={13}
                                                    color="var(--color-error)"
                                                /></span
                                            >
                                        {:else}
                                            <span class="inline-flex"
                                                ><Icon
                                                    name="check"
                                                    size={13}
                                                    color="var(--color-ink-3)"
                                                /></span
                                            >
                                        {/if}
                                    {/if}
                                </div>
                                {#if message.local_sender && message.deleted_at === 0}
                                    <div
                                        class="flex justify-start gap-1.5 group-data-local:justify-end"
                                    >
                                        {#if message.state === 'failed' || message.state === 'expired'}
                                            <button
                                                class="inline-flex min-h-7 items-center justify-center rounded-pill border border-line bg-transparent px-3 text-xs leading-none font-bold text-ink-2 transition-colors duration-150 ease-soft hover:border-line-strong hover:bg-surface hover:text-ink disabled:cursor-not-allowed disabled:opacity-60"
                                                disabled={app.messageRetryPending === message.id}
                                                onclick={() => void app.retryMessage(message.id)}
                                            >
                                                {app.messageRetryPending === message.id
                                                    ? 'Retrying…'
                                                    : 'Retry'}
                                            </button>
                                        {:else if message.body !== '' && message.attachment_hash === ''}
                                            <button
                                                class="inline-flex min-h-7 items-center justify-center rounded-pill border border-line bg-transparent px-3 text-xs leading-none font-bold text-ink-2 transition-colors duration-150 ease-soft hover:border-line-strong hover:bg-surface hover:text-ink"
                                                onclick={() => openEdit(message)}>Edit</button
                                            >
                                        {/if}
                                        <button
                                            class="inline-flex min-h-7 items-center justify-center rounded-pill border border-line bg-transparent px-3 text-xs leading-none font-bold text-error transition-colors duration-150 ease-soft hover:border-error hover:bg-error/10"
                                            onclick={() => {
                                                deleteTarget = message;
                                                deleteError = '';
                                            }}>Delete</button
                                        >
                                    </div>
                                {/if}
                            </article>
                        {/if}
                    {/if}
                {/each}
                {#if timeline.length === 0}
                    <div
                        class="flex min-h-full flex-col items-center justify-center gap-2 text-center"
                    >
                        <p
                            class="m-0 font-display text-step-3 leading-[1.15] font-normal tracking-[-0.01em] text-ink"
                        >
                            A quiet line is still open.
                        </p>
                        <p class="m-0 text-step-xs text-ink-3">
                            Say hello — messages stay on this network.
                        </p>
                    </div>
                {/if}
            {/if}
        </div>
        <div class="h-[18px] shrink-0 px-7 text-xs leading-4 text-ink-3" aria-live="polite">
            {#if app.typing[selectedUser.id]}
                <span>{shortName(selectedUser)} is typing…</span>
            {/if}
        </div>
        {#if composerBlocked !== '' || overLimit}
            <div class="mx-5 mb-2">
                <StateBanner
                    tone={overLimit ? 'error' : 'warning'}
                    title={overLimit ? 'Message is too long' : composerBlocked}
                    body={overLimit
                        ? `${draftBytes} of 32768 bytes. Shorten the message before sending.`
                        : 'Your draft is kept until you can send again.'}
                />
            </div>
        {/if}
        <form
            class="mx-5 mb-5 flex shrink-0 flex-col rounded-md border border-line bg-surface px-4 pt-3.5 pb-2.5"
            onsubmit={send}
        >
            {#if app.recording}
                <div class="flex items-center gap-3" role="status">
                    <span class="size-2.5 shrink-0 animate-pulse rounded-full bg-error"></span>
                    <span class="text-sm font-bold text-ink tabular-nums"
                        >{sessionClock(app.recordingSeconds)}</span
                    >
                    <span
                        bind:this={recordingTrack}
                        class="flex h-8 min-w-0 flex-1 items-center justify-start gap-[3px]"
                        aria-hidden="true"
                    >
                        {#each recordingLevels as level, index (index)}
                            <span
                                class="w-[3px] shrink-0 rounded-full bg-ink-2"
                                style="height: {6 + Math.min(1, Math.max(0, level)) * 26}px;"
                            ></span>
                        {/each}
                    </span>
                    <button
                        type="button"
                        class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-error hover:bg-error/10"
                        onclick={() => app.cancelRecording()}
                        aria-label="Cancel voice message"
                    >
                        <Icon name="close" size={16} color="var(--color-error)" />
                    </button>
                    <button
                        type="button"
                        class="flex size-8 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent"
                        onclick={() => void app.stopRecording()}
                        aria-label="Stop and send voice message"
                    >
                        <Icon name="check" size={16} color="var(--color-base)" />
                    </button>
                </div>
            {:else}
                <label class="sr-only" for="message-body">Message</label>
                <textarea
                    id="message-body"
                    class="max-h-[120px] min-h-9 w-full resize-none border-0 bg-transparent text-step-0 leading-[1.45] text-ink outline-none placeholder:text-ink-3"
                    bind:this={composer}
                    value={app.draft}
                    disabled={composerBlocked !== ''}
                    oninput={(event) =>
                        app.updateDraft((event.currentTarget as HTMLTextAreaElement).value)}
                    onkeydown={onComposerKey}
                    placeholder={composerBlocked === '' ? 'Write a message' : composerBlocked}
                    rows="1"></textarea>
                <div class="flex items-center gap-1.5">
                    <input class="sr-only" type="file" bind:this={fileInput} onchange={attach} />
                    <button
                        type="button"
                        class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50"
                        disabled={composerBlocked !== ''}
                        onclick={() => fileInput?.click()}
                        aria-label="Attach file"
                    >
                        <Icon name="paperclip" size={16} color="var(--color-ink-2)" />
                    </button>
                    <span
                        class="ml-auto text-xs tabular-nums {overLimit
                            ? 'text-error'
                            : 'text-ink-3'}"
                    >
                        {app.draft === '' ? '' : `${draftBytes} / 32768`}
                    </span>
                    {#if app.draft.trim() === ''}
                        <button
                            type="button"
                            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50"
                            disabled={composerBlocked !== ''}
                            onclick={() => void app.startRecording()}
                            aria-label="Record voice message"
                        >
                            <Icon name="mic" size={16} color="var(--color-ink-2)" />
                        </button>
                    {:else}
                        <button
                            type="submit"
                            class="flex h-8 w-10 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                            aria-label="Send message"
                            disabled={!canSend()}
                        >
                            <Icon name="send" size={18} color="var(--color-base)" />
                        </button>
                    {/if}
                </div>
            {/if}
        </form>
    {:else}
        <div class="flex min-h-full flex-col items-center justify-center gap-2 text-center">
            <p
                class="m-0 font-display text-step-3 leading-[1.15] font-normal tracking-[-0.01em] text-ink"
            >
                Pick a conversation.
            </p>
            <p class="m-0 text-step-xs text-ink-3">
                The rail lists everyone discovered on this LAN.
            </p>
        </div>
    {/if}
    {#if preview}
        <div
            class="fixed inset-0 z-[60] flex items-center justify-center bg-backdrop/80 p-8"
            role="dialog"
            aria-modal="true"
            aria-label={preview.name}
        >
            <button
                class="absolute inset-0 cursor-default"
                onclick={() => (preview = null)}
                aria-label="Close image preview"
            ></button>
            <button
                class="absolute top-5 right-5 z-10 inline-flex size-9 items-center justify-center rounded-pill border border-line bg-surface text-ink"
                onclick={() => (preview = null)}
                aria-label="Close image"
            >
                <Icon name="close" size={16} />
            </button>
            <img
                class="relative z-10 max-h-full max-w-full rounded-md object-contain shadow-soft"
                src={preview.url}
                alt={preview.name}
            />
        </div>
    {/if}
    <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete message"
        body="Delete this message for you? This cannot be undone."
        confirmLabel="Delete message"
        cancelLabel="Cancel"
        tone="error"
        pending={app.messageActionPending !== ''}
        error={deleteError}
        onconfirm={() => void confirmDelete()}
        oncancel={closeDelete}
    />
    <EditMessageDialog
        open={editingMessage !== null}
        initial={editingBody}
        pending={app.messageActionPending !== ''}
        error={editError}
        onsave={(body) => void saveEdit(body)}
        oncancel={closeEdit}
    />
</section>

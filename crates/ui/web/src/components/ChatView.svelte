<script lang="ts">
    import { tick } from 'svelte';

    import type { AppState, CallLog, MenuAnchor, Message } from '../lib';
    import {
        attachmentKind,
        clock,
        dayLabel,
        fileSize,
        fingerprintLabel,
        messageMenuAlignment,
        placeMessageMenu,
        sessionClock,
        shortName,
    } from '../lib';
    import { firstVideoId } from '../lib';
    import Avatar from './Avatar.svelte';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import Icon from './Icon.svelte';
    import MessageComposer from './MessageComposer.svelte';
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
    let imageUrls = $state<Record<string, string>>({});
    let imageLoading = $state<Record<string, boolean>>({});
    let imageErrors = $state<Record<string, string>>({});
    let preview = $state<{ url: string; name: string } | null>(null);
    let editingMessage = $state<Message | null>(null);
    let editingBody = $state('');
    let editError = $state('');
    let editTextarea = $state<HTMLTextAreaElement | null>(null);
    let deleteTarget = $state<Message | null>(null);
    let deleteError = $state('');
    let openMenuMessageId = $state<string | null>(null);
    let menuPosition = $state<MenuAnchor | null>(null);
    let replyTarget = $state<Message | null>(null);
    let highlightedMessageId = $state<string | null>(null);
    let highlightTimer: ReturnType<typeof setTimeout> | null = null;
    const EDIT_WINDOW_MS = 5 * 60 * 1000;
    let prependingOlderMessages = false;
    const loadingTimelineRows = [0, 1, 2, 3, 4];

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

    function openEdit(message: Message): void {
        editingMessage = message;
        editingBody = message.body;
        editError = '';
        openMenuMessageId = null;
    }

    function toggleMessageMenu(message: Message, event: MouseEvent): void {
        if (openMenuMessageId === message.id) {
            closeMessageMenu();
            return;
        }
        openMenuMessageId = message.id;
        const anchor = event.currentTarget;
        if (anchor instanceof HTMLElement) {
            menuPosition = placeMessageMenu(
                window.innerWidth,
                window.innerHeight,
                anchor.getBoundingClientRect(),
                messageMenuAlignment(message.local_sender),
            );
        } else {
            menuPosition = null;
        }
    }

    function closeMessageMenu(): void {
        openMenuMessageId = null;
        menuPosition = null;
    }

    function requestDelete(message: Message): void {
        deleteTarget = message;
        deleteError = '';
        openMenuMessageId = null;
    }

    function canShowActions(message: Message): boolean {
        if (message.deleted_at !== 0 || editingMessage?.id === message.id) {
            return false;
        }
        if (!message.local_sender) {
            return true;
        }
        return message.state !== 'failed' && message.state !== 'expired';
    }

    function canEdit(message: Message): boolean {
        if (!message.local_sender || message.deleted_at !== 0) {
            return false;
        }
        if (message.body === '' || message.attachment_hash !== '') {
            return false;
        }
        const at = message.sent_at || message.received_at;
        return Date.now() - at < EDIT_WINDOW_MS;
    }

    function requestReply(message: Message): void {
        replyTarget = message;
        app.replyToId = message.id;
        openMenuMessageId = null;
    }

    function cancelReply(): void {
        replyTarget = null;
        app.replyToId = '';
    }

    function replyOriginal(message: Message): Message | undefined {
        if (message.reply_to === '') {
            return undefined;
        }
        return app.messages.find((item) => item.id === message.reply_to);
    }

    function replySenderName(target: Message): string {
        const user =
            app.userById(target.sender_id) ??
            (selectedUser?.id === target.sender_id ? selectedUser : null);
        if (user === null) {
            return 'Peer';
        }
        return shortName(user);
    }

    function replyLabel(target: Message | undefined): string {
        if (target === undefined) {
            return 'Original message';
        }
        return target.local_sender ? 'You' : replySenderName(target);
    }

    function replySnippet(target: Message | undefined): string {
        if (target === undefined) {
            return 'Original message unavailable';
        }
        if (target.deleted_at > 0) {
            return 'Original message deleted';
        }
        const text = target.body.trim();
        if (text !== '') {
            return text.length > 140 ? `${text.slice(0, 140)}…` : text;
        }
        if (target.attachment_hash !== '') {
            const kind = attachmentKind(target.attachment_mime, target.attachment_name);
            if (kind === 'image') {
                return target.attachment_name || 'Image';
            }
            if (kind === 'audio') {
                return target.attachment_name || 'Voice message';
            }
            return target.attachment_name || 'Attachment';
        }
        return 'Message unavailable.';
    }

    async function jumpToMessage(messageId: string): Promise<void> {
        const element = document.getElementById(`chat-message-${messageId}`);
        if (element === null) {
            return;
        }
        element.scrollIntoView({ behavior: 'smooth', block: 'center' });
        if (highlightTimer !== null) {
            clearTimeout(highlightTimer);
            highlightTimer = null;
        }
        highlightedMessageId = null;
        await tick();
        highlightedMessageId = messageId;
        highlightTimer = setTimeout(() => {
            highlightedMessageId = null;
            highlightTimer = null;
        }, 1600);
    }

    $effect(() => {
        if (app.replyToId === '') {
            replyTarget = null;
        }
    });

    $effect(() => {
        app.selectedUserId;
        cancelReply();
    });

    function handleDocumentClick(event: MouseEvent): void {
        if (openMenuMessageId === null) {
            return;
        }
        if (!(event.target instanceof HTMLElement)) {
            return;
        }
        if (event.target.closest('[data-message-menu]') !== null) {
            return;
        }
        openMenuMessageId = null;
    }

    function handleMenuKeydown(event: KeyboardEvent): void {
        if (event.key === 'Escape') {
            openMenuMessageId = null;
        }
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

    function editInputKeydown(event: KeyboardEvent): void {
        if (event.key === 'Escape') {
            event.preventDefault();
            closeEdit();
            return;
        }
        if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
            return;
        }
        event.preventDefault();
        if (editingBody.trim() === '' || app.messageActionPending !== '') {
            return;
        }
        void saveEdit(editingBody);
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
        closeMessageMenu();
        void loadOlderMessages();
    }

    $effect(() => {
        if (editingMessage === null) {
            return;
        }
        const frame = window.requestAnimationFrame(() => {
            editTextarea?.focus();
            const caret = editTextarea?.value.length ?? 0;
            editTextarea?.setSelectionRange(caret, caret);
        });
        return () => window.cancelAnimationFrame(frame);
    });

    $effect(() => {
        if (timeline.length > 0 && stream !== null && !prependingOlderMessages) {
            stream.scrollTop = stream.scrollHeight;
        }
    });
</script>

<svelte:document onclick={handleDocumentClick} />

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
                                id="chat-message-{message.id}"
                                class="group flex max-w-[min(760px,78%)] flex-row-reverse items-start gap-3 self-start rounded-md pb-3 text-ink data-local:flex-row data-local:self-end {highlightedMessageId ===
                                message.id
                                    ? 'reply-flash -mx-3 -my-3 rounded-md px-3 py-3'
                                    : ''}"
                                class:w-full={editingMessage?.id === message.id}
                                data-local={message.local_sender ? '' : undefined}
                            >
                                {#if canShowActions(message)}
                                    <div
                                        class="flex shrink-0 self-center"
                                        data-message-menu={message.id}
                                    >
                                        <button
                                            class="inline-flex size-7 items-center justify-center rounded-pill border border-line bg-surface-2 text-ink-2 opacity-0 transition-[background-color,border-color,color,opacity] duration-150 ease-soft group-hover:opacity-100 hover:border-line-strong hover:text-ink focus-visible:opacity-100 {openMenuMessageId ===
                                            message.id
                                                ? 'opacity-100'
                                                : ''}"
                                            aria-label="Message actions"
                                            aria-haspopup="menu"
                                            aria-expanded={openMenuMessageId === message.id}
                                            onclick={(event) => toggleMessageMenu(message, event)}
                                        >
                                            <Icon name="dots" size={16} />
                                        </button>
                                        {#if openMenuMessageId === message.id && menuPosition !== null}
                                            <div
                                                class="fixed z-50 flex w-40 flex-col gap-0.5 rounded-md border border-line bg-surface-2 p-1 shadow-soft"
                                                style="left: {menuPosition.left}px; top: {menuPosition.top}px;"
                                                role="menu"
                                                tabindex={-1}
                                                data-message-menu={message.id}
                                                onkeydown={handleMenuKeydown}
                                            >
                                                {#if message.deleted_at === 0}
                                                    <button
                                                        class="flex w-full items-center rounded-sm px-3 py-2 text-left text-step-xs font-bold text-ink hover:bg-surface"
                                                        role="menuitem"
                                                        onclick={() => requestReply(message)}
                                                    >
                                                        Reply
                                                    </button>
                                                {/if}
                                                {#if canEdit(message)}
                                                    <button
                                                        class="flex w-full items-center rounded-sm px-3 py-2 text-left text-step-xs font-bold text-ink hover:bg-surface"
                                                        role="menuitem"
                                                        onclick={() => openEdit(message)}
                                                    >
                                                        Edit
                                                    </button>
                                                {/if}
                                                {#if message.local_sender}
                                                    <button
                                                        class="flex w-full items-center rounded-sm px-3 py-2 text-left text-step-xs font-bold text-error hover:bg-error/10"
                                                        role="menuitem"
                                                        onclick={() => requestDelete(message)}
                                                    >
                                                        Delete
                                                    </button>
                                                {/if}
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                                <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                                    {#if message.deleted_at > 0}
                                        <p class="m-0 text-step-xs leading-[1.5] text-ink-3 italic">
                                            Message deleted
                                        </p>
                                    {:else}
                                        {#if editingMessage?.id === message.id}
                                            <div class="flex w-full flex-col gap-2">
                                                <div
                                                    class="flex items-center justify-between gap-2"
                                                >
                                                    <span
                                                        class="text-xs font-bold tracking-[0.06em] text-ink-2 uppercase"
                                                        >Editing</span
                                                    >
                                                    <span class="text-xs text-ink-3"
                                                        >Enter to save · Esc to cancel</span
                                                    >
                                                </div>
                                                <label
                                                    class="sr-only"
                                                    for="edit-message-{message.id}"
                                                    >Message text</label
                                                >
                                                <textarea
                                                    id="edit-message-{message.id}"
                                                    class="min-h-[96px] w-full resize-y rounded-sm border border-line-strong bg-base px-3 py-2 text-step-0 leading-[1.45] text-ink outline-none"
                                                    bind:this={editTextarea}
                                                    bind:value={editingBody}
                                                    maxlength="32768"
                                                    disabled={app.messageActionPending !== ''}
                                                    rows="3"
                                                    onkeydown={editInputKeydown}></textarea>
                                                <div
                                                    class="flex items-center justify-between gap-2"
                                                >
                                                    <span
                                                        class="text-xs text-ink-3"
                                                        role={editError ? 'alert' : undefined}
                                                    >
                                                        {editError ||
                                                            `${editingBody.length} / 32768`}
                                                    </span>
                                                    <div class="flex gap-2">
                                                        <button
                                                            type="button"
                                                            class="inline-flex min-h-9 items-center justify-center rounded-pill border border-line bg-transparent px-4 text-step-xs leading-none font-bold text-ink hover:border-line-strong hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-60"
                                                            disabled={app.messageActionPending !==
                                                                ''}
                                                            onclick={closeEdit}>Cancel</button
                                                        >
                                                        <button
                                                            type="button"
                                                            class="inline-flex min-h-9 items-center justify-center rounded-pill border border-accent bg-accent px-4 text-step-xs leading-none font-bold text-base hover:border-ink-2 hover:bg-ink-2 disabled:cursor-not-allowed disabled:opacity-50"
                                                            disabled={app.messageActionPending !==
                                                                '' || editingBody.trim() === ''}
                                                            onclick={() =>
                                                                void saveEdit(editingBody)}
                                                            >{app.messageActionPending !== ''
                                                                ? 'Saving…'
                                                                : 'Save'}</button
                                                        >
                                                    </div>
                                                </div>
                                            </div>
                                        {:else}
                                            {#if message.reply_to !== ''}
                                                {@const original = replyOriginal(message)}
                                                <button
                                                    class="flex w-full min-w-0 flex-col gap-0.5 rounded-sm border-l-2 border-accent bg-surface-2/60 px-2.5 py-1.5 text-left hover:bg-surface-2"
                                                    onclick={() =>
                                                        void jumpToMessage(message.reply_to)}
                                                    aria-label="Jump to original message"
                                                >
                                                    <span class="text-xs font-bold text-accent"
                                                        >{replyLabel(original)}</span
                                                    >
                                                    <span class="truncate text-xs text-ink-3"
                                                        >{replySnippet(original)}</span
                                                    >
                                                </button>
                                            {/if}
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
                                                            )}{transferLabel(
                                                                message.attachment_hash,
                                                            )
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
                                                                message.attachment_name ||
                                                                    'attachment',
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
                                                                )}{transferLabel(
                                                                    message.attachment_hash,
                                                                )
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
                                                    <div
                                                        class="flex min-w-0 flex-col items-start gap-1"
                                                    >
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
                                    {#if message.local_sender && message.deleted_at === 0 && (message.state === 'failed' || message.state === 'expired')}
                                        <div
                                            class="flex justify-start gap-1.5 group-data-local:justify-end"
                                        >
                                            <button
                                                class="inline-flex min-h-7 items-center justify-center rounded-pill border border-line bg-transparent px-3 text-xs leading-none font-bold text-ink-2 transition-colors duration-150 ease-soft hover:border-line-strong hover:bg-surface hover:text-ink disabled:cursor-not-allowed disabled:opacity-60"
                                                disabled={app.messageRetryPending === message.id}
                                                onclick={() => void app.retryMessage(message.id)}
                                            >
                                                {app.messageRetryPending === message.id
                                                    ? 'Retrying…'
                                                    : 'Retry'}
                                            </button>
                                        </div>
                                    {/if}
                                </div>
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
        <div class="mx-5 mb-5 shrink-0">
            {#if replyTarget !== null}
                <div
                    class="mb-2 flex items-center gap-2 rounded-md border border-line bg-surface px-3 py-2"
                >
                    <div class="min-w-0 flex-1 border-l-2 border-accent pl-2">
                        <p class="m-0 truncate text-xs font-bold text-accent">
                            Replying to {replyLabel(replyTarget)}
                        </p>
                        <p class="m-0 truncate text-xs text-ink-3">
                            {replySnippet(replyTarget)}
                        </p>
                    </div>
                    <button
                        type="button"
                        class="inline-flex size-7 shrink-0 items-center justify-center rounded-pill text-ink-2 hover:bg-surface-2 hover:text-ink"
                        onclick={cancelReply}
                        aria-label="Cancel reply"
                    >
                        <Icon name="close" size={14} />
                    </button>
                </div>
            {/if}
            <MessageComposer {app} />
        </div>
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
        body="Delete this message for everyone? This cannot be undone."
        confirmLabel="Delete message"
        cancelLabel="Cancel"
        tone="error"
        pending={app.messageActionPending !== ''}
        error={deleteError}
        onconfirm={() => void confirmDelete()}
        oncancel={closeDelete}
    />
</section>

<style>
    .reply-flash {
        animation: reply-flash 1.6s ease-out forwards;
    }

    @keyframes reply-flash {
        from {
            background-color: color-mix(in srgb, var(--color-accent) 16%, transparent);
        }
        to {
            background-color: transparent;
        }
    }
</style>

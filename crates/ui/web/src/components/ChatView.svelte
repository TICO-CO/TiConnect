<script lang="ts">
    import { tick } from 'svelte';

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
    let prependingOlderMessages = false;

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
        if (app.messages.length > 0 && stream !== null && !prependingOlderMessages) {
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
            <div class="flex shrink-0 gap-2.5">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={() => void app.startCall(selectedUser.id)}
                >
                    Call
                    <Icon name="call" size={15} />
                </button>
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={() => void app.requestRemote(selectedUser.id)}
                >
                    Remote desktop
                    <Icon name="remote" size={15} />
                </button>
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    onclick={playTogether}
                >
                    Watch party
                    <Icon name="chevron-right" size={13} />
                </button>
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
            {#if app.loadingOlderMessages}
                <div
                    class="flex shrink-0 justify-center py-2 text-step-xs text-ink-3"
                    role="status"
                >
                    Loading older messages…
                </div>
            {/if}
            {#each app.messages as message, index (message.id)}
                {@const previous = app.messages[index - 1]}
                {@const showDay =
                    previous === undefined ||
                    dayLabel(previous.sent_at) !== dayLabel(message.sent_at)}
                {#if showDay}
                    <div class="flex items-center justify-center pt-2 pb-3.5">
                        <span
                            class="rounded-pill border border-line px-3 py-1 text-xs tracking-[0.06em] text-ink-3"
                            >{dayLabel(message.sent_at)}</span
                        >
                    </div>
                {/if}
                {#if message.body === '' && message.attachment_hash === ''}
                    <p class="m-0 text-step-xs text-ink-3">Message unavailable.</p>
                {:else}
                    <article
                        class="group flex max-w-[min(760px,78%)] flex-col gap-1.5 self-start pb-3 text-ink data-local:self-end"
                        data-local={message.local_sender ? '' : undefined}
                    >
                        {#if message.body && !isOnlyVideoLink(message.body)}
                            <p
                                class="m-0 text-step-0 leading-[155%] [overflow-wrap:anywhere] whitespace-pre-wrap"
                            >
                                {message.body}
                            </p>
                        {/if}
                        {#if message.attachment_hash}
                            <button
                                class="flex w-full items-center gap-3 rounded-md border border-line bg-surface px-3.5 py-3 text-left text-ink hover:border-line-strong hover:bg-surface-2"
                                onclick={() => void app.openAttachment(message.attachment_hash)}
                            >
                                <span
                                    class="flex size-8 shrink-0 items-center justify-center rounded-sm bg-base"
                                    ><Icon name="file" size={18} color="var(--color-ink-2)" /></span
                                >
                                <span class="flex flex-col gap-0.5">
                                    <strong class="text-[14px]"
                                        >{message.attachment_name || 'Attachment'}</strong
                                    >
                                    <span class="text-xs text-ink-3"
                                        >BLAKE3 {fingerprintLabel(message.attachment_hash)} · {fileSize(
                                            message.attachment_size,
                                        )}</span
                                    >
                                </span>
                            </button>
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
                                    <span class="absolute inset-0 flex items-center justify-center"
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
                                    <span class="text-xs text-ink-3">TiConnect Labs · 18:42</span>
                                    <span class="text-xs tracking-[0.05em] text-ink-2 uppercase"
                                        >Playing in overlay · 2 in queue</span
                                    >
                                    <button
                                        class="mt-1 inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                                        onclick={() =>
                                            void app.startParty(videoId, selectedUser.id)}
                                    >
                                        Watch party
                                        <Icon name="chevron-right" size={12} />
                                    </button>
                                </div>
                            </div>
                        {/if}
                        <div
                            class="flex items-center justify-start gap-[5px] text-xs leading-4 text-ink-3 tabular-nums group-data-local:justify-end"
                        >
                            <span>{clock(message.sent_at || message.received_at)}</span>
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
                                {:else if message.state === 'failed'}
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
                    </article>
                {/if}
            {/each}
            {#if callLogs.length > 0}
                {#each callLogs as log (log.id)}
                    <div
                        class="flex w-[min(720px,100%)] items-center justify-center gap-3 self-center py-2"
                    >
                        <span class="h-px flex-1 bg-line"></span>
                        <span
                            class="flex items-center gap-1.5 text-xs tracking-[0.04em] text-ink-3"
                        >
                            <Icon name="call" size={12} color="var(--color-ink-3)" />
                            {log.reason === 'missed' ? 'Missed call' : 'Call ended'} · {sessionClock(
                                log.durationSeconds,
                            )}
                        </span>
                        <span class="h-px flex-1 bg-line"></span>
                    </div>
                {/each}
            {/if}
            {#if app.typing[selectedUser.id]}
                <p class="m-0 text-xs text-ink-3">{shortName(selectedUser)} is typing…</p>
            {/if}
            {#if app.messages.length === 0}
                <div class="flex min-h-full flex-col items-center justify-center gap-2 text-center">
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
        </div>
        <form
            class="mx-5 mb-5 flex shrink-0 flex-col rounded-md border border-line bg-surface px-4 pt-3.5 pb-2.5"
            onsubmit={send}
        >
            <label class="sr-only" for="message-body">Message</label>
            <textarea
                id="message-body"
                class="max-h-[120px] min-h-9 w-full resize-none border-0 bg-transparent text-step-0 leading-[1.45] text-ink outline-none placeholder:text-ink-3"
                bind:this={composer}
                value={app.draft}
                oninput={(event) =>
                    app.updateDraft((event.currentTarget as HTMLTextAreaElement).value)}
                onkeydown={onComposerKey}
                placeholder="Write a message"
                rows="1"></textarea>
            <div class="flex items-center gap-1.5">
                <input class="sr-only" type="file" bind:this={fileInput} onchange={attach} />
                <button
                    type="button"
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
                    onclick={() => fileInput?.click()}
                    aria-label="Attach file"
                >
                    <Icon name="paperclip" size={16} color="var(--color-ink-2)" />
                </button>
                <button
                    type="button"
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface-2"
                    onclick={() =>
                        (app.notice =
                            'Voice recording is not wired yet. Attach an audio file instead.')}
                    aria-label="Record voice message"
                >
                    <Icon name="mic" size={16} color="var(--color-ink-2)" />
                </button>
                <button
                    type="submit"
                    class="ml-auto flex h-8 w-10 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent"
                    aria-label="Send message"
                >
                    <Icon name="send" size={18} color="var(--color-base)" />
                </button>
            </div>
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
</section>

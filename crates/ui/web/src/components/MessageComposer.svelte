<script lang="ts">
    import type { AppState } from '../lib';
    import { sessionClock } from '../lib';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let fileInput: HTMLInputElement | null = $state(null);
    let composer: HTMLTextAreaElement | null = $state(null);
    let recordingTrack: HTMLElement | null = $state(null);
    let recordingTrackWidth = $state(0);
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

    function canSend(): boolean {
        return (
            composerBlocked === '' &&
            !overLimit &&
            app.draft.trim() !== '' &&
            app.connection === 'online'
        );
    }

    function send(event: SubmitEvent): void {
        event.preventDefault();
        if (!canSend()) {
            return;
        }
        void app.sendMessage();
        composer?.focus();
    }

    function onComposerKey(event: KeyboardEvent): void {
        if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
            return;
        }
        event.preventDefault();
        if (canSend()) {
            void app.sendMessage();
        }
    }

    function resizeComposer(): void {
        if (composer === null) {
            return;
        }
        composer.style.height = 'auto';
        composer.style.height = `${composer.scrollHeight}px`;
    }

    function attach(event: Event): void {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (file) {
            void app.sendFile(file);
        }
        input.value = '';
    }

    $effect(() => {
        app.draft;
        resizeComposer();
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
</script>

{#if app.recording}
    <div class="flex w-full items-center gap-3">
        <div
            class="flex min-h-12 flex-1 items-center gap-2.5 rounded-pill border border-line bg-surface py-3 pr-3 pl-4"
        >
            <span class="size-2.5 shrink-0 animate-pulse rounded-full bg-error"></span>
            <span
                class="shrink-0 text-sm font-bold text-ink tabular-nums"
                role="timer"
                aria-live="off">{sessionClock(app.recordingSeconds)}</span
            >
            <span
                bind:this={recordingTrack}
                class="flex h-8 min-w-0 flex-1 items-center justify-start gap-[3px]"
                aria-hidden="true"
            >
                {#each recordingLevels as level, index (index)}
                    {@const live = index >= recordingLevels.length - 2}
                    <span
                        class="w-[3px] shrink-0 rounded-full {live ? 'bg-accent' : 'bg-ink-3/70'}"
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
                <Icon name="trash" size={16} color="var(--color-error)" />
            </button>
        </div>
        <button
            type="button"
            class="flex size-12 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent"
            onclick={() => void app.stopRecording()}
            aria-label="Stop and send voice message"
        >
            <Icon name="send" size={18} color="var(--color-base)" />
        </button>
    </div>
{:else}
    <form class="flex w-full items-center gap-3" onsubmit={send}>
        <div
            class="flex min-h-12 flex-1 items-center gap-2 rounded-pill border border-line bg-surface py-4 pr-3 pl-6"
        >
            <label class="sr-only" for="message-body">Message</label>
            <textarea
                id="message-body"
                class="max-h-[4.35em] w-full flex-1 resize-none self-center overflow-y-auto border-0 bg-transparent text-step-0 leading-[1.45] text-ink outline-none placeholder:text-ink-3"
                bind:this={composer}
                value={app.draft}
                disabled={composerBlocked !== ''}
                oninput={(event) =>
                    app.updateDraft((event.currentTarget as HTMLTextAreaElement).value)}
                onkeydown={onComposerKey}
                placeholder={composerBlocked === '' ? 'Write a message' : composerBlocked}
                rows="1"></textarea>
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
        </div>
        {#if app.draft.trim() === ''}
            <button
                type="button"
                class="flex size-12 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                disabled={composerBlocked !== ''}
                onclick={() => void app.startRecording()}
                aria-label="Record voice message"
            >
                <Icon name="mic" size={18} color="var(--color-base)" />
            </button>
        {:else}
            <button
                type="submit"
                class="flex size-12 shrink-0 items-center justify-center rounded-pill border border-accent bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                aria-label="Send message"
                disabled={!canSend()}
            >
                <Icon name="send" size={18} color="var(--color-base)" />
            </button>
        {/if}
    </form>
{/if}

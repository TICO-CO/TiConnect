<script lang="ts">
    interface Props {
        open: boolean;
        initial: string;
        pending: boolean;
        error?: string;
        onsave: (body: string) => void;
        oncancel: () => void;
    }

    let { open, initial, pending, error = '', onsave, oncancel }: Props = $props();
    let draft = $state('');
    let textarea: HTMLTextAreaElement | null = $state(null);

    $effect(() => {
        if (!open) {
            return;
        }
        draft = initial;
        const frame = window.requestAnimationFrame(() => {
            textarea?.focus();
            textarea?.setSelectionRange(draft.length, draft.length);
        });
        return () => window.cancelAnimationFrame(frame);
    });

    function submit(event: SubmitEvent): void {
        event.preventDefault();
        if (draft.trim() !== '' && !pending) {
            onsave(draft);
        }
    }

    function onKeydown(event: KeyboardEvent): void {
        if (event.key === 'Escape' && !pending) {
            event.preventDefault();
            oncancel();
        }
    }
</script>

{#if open}
    <div
        class="fixed inset-0 z-[60] flex items-center justify-center bg-backdrop/70 p-6"
        role="presentation"
        onkeydown={onKeydown}
    >
        <div
            class="w-[520px] max-w-full rounded-lg border border-line-strong bg-surface p-5 shadow-soft"
            role="dialog"
            aria-modal="true"
            aria-labelledby="edit-message-title"
        >
            <form class="flex flex-col" onsubmit={submit}>
                <h2
                    id="edit-message-title"
                    class="m-0 font-display text-step-1 leading-[1.2] font-normal tracking-[-0.01em]"
                >
                    Edit message
                </h2>
                <label class="sr-only" for="edit-message-body">Message text</label>
                <textarea
                    id="edit-message-body"
                    class="mt-3 min-h-[96px] w-full resize-y rounded-sm border border-line bg-base px-3 py-2 text-step-0 leading-[1.45] text-ink outline-none focus:border-line-strong"
                    bind:this={textarea}
                    bind:value={draft}
                    maxlength="32768"
                    disabled={pending}
                    rows="3"></textarea>
                <div class="mt-2 flex items-center justify-between gap-2">
                    <span class="text-xs text-ink-3" role={error ? 'alert' : undefined}>
                        {error || `${draft.length} / 32768`}
                    </span>
                    <div class="flex gap-2">
                        <button
                            type="button"
                            class="inline-flex min-h-9 items-center justify-center rounded-pill border border-line bg-transparent px-4 text-step-xs leading-none font-bold text-ink hover:border-line-strong hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-60"
                            disabled={pending}
                            onclick={oncancel}>Cancel</button
                        >
                        <button
                            type="submit"
                            class="inline-flex min-h-9 items-center justify-center rounded-pill border border-accent bg-accent px-4 text-step-xs leading-none font-bold text-base hover:border-ink-2 hover:bg-ink-2 disabled:cursor-not-allowed disabled:opacity-50"
                            aria-busy={pending}
                            disabled={pending || draft.trim() === ''}
                            >{pending ? 'Saving…' : 'Save'}</button
                        >
                    </div>
                </div>
            </form>
        </div>
    </div>
{/if}

<script lang="ts">
    import type { StateTone } from '../lib';

    interface Props {
        open: boolean;
        title: string;
        body: string;
        confirmLabel: string;
        cancelLabel: string;
        tone: StateTone;
        pending: boolean;
        error: string;
        onconfirm: () => void;
        oncancel: () => void;
    }

    let {
        open,
        title,
        body,
        confirmLabel,
        cancelLabel,
        tone,
        pending,
        error,
        onconfirm,
        oncancel,
    }: Props = $props();
    let panel: HTMLDivElement | null = $state(null);
    let previousFocus: HTMLElement | null = null;

    const confirmStyles = {
        neutral: 'border-accent bg-accent text-base hover:border-ink-2 hover:bg-ink-2',
        info: 'border-accent bg-accent text-base hover:border-ink-2 hover:bg-ink-2',
        success: 'border-success bg-success text-base hover:opacity-90',
        warning: 'border-warning bg-warning text-backdrop hover:opacity-90',
        error: 'border-error bg-error text-base hover:opacity-90',
    } satisfies Record<StateTone, string>;
    const confirmStyle = $derived(confirmStyles[tone]);

    $effect(() => {
        if (!open) {
            return;
        }
        previousFocus =
            document.activeElement instanceof HTMLElement ? document.activeElement : null;
        const frame = window.requestAnimationFrame(() => panel?.focus());
        return () => {
            window.cancelAnimationFrame(frame);
            previousFocus?.focus();
        };
    });

    function focusableElements(): HTMLElement[] {
        if (panel === null) {
            return [];
        }
        return Array.from(
            panel.querySelectorAll<HTMLElement>(
                'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
            ),
        );
    }

    function onKeydown(event: KeyboardEvent): void {
        if (event.key === 'Escape') {
            event.preventDefault();
            oncancel();
            return;
        }
        if (event.key !== 'Tab') {
            return;
        }
        const focusable = focusableElements();
        const first = focusable.at(0);
        const last = focusable.at(-1);
        if (first === undefined || last === undefined) {
            event.preventDefault();
            panel?.focus();
            return;
        }
        if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
            return;
        }
        if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
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
            class="flex w-[420px] max-w-full flex-col gap-0 rounded-lg border border-line-strong bg-surface p-5 shadow-soft"
            bind:this={panel}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="confirm-dialog-title"
            aria-describedby="confirm-dialog-body"
            tabindex="-1"
        >
            <h2
                id="confirm-dialog-title"
                class="m-0 font-display text-step-1 leading-[1.2] font-normal tracking-[-0.01em]"
            >
                {title}
            </h2>
            <p id="confirm-dialog-body" class="mt-2 mb-0 text-step-xs leading-[1.5] text-ink-2">
                {body}
            </p>
            {#if error}
                <p class="mt-2 mb-0 text-step-xs leading-[1.5] text-error" role="alert">{error}</p>
            {/if}
            <div class="mt-5 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-9 items-center justify-center rounded-pill border border-line bg-transparent px-4 text-step-xs leading-none font-bold text-ink hover:border-line-strong hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={pending}
                    onclick={oncancel}>{cancelLabel}</button
                >
                <button
                    class="inline-flex min-h-9 items-center justify-center rounded-pill border px-4 text-step-xs leading-none font-bold transition-[background-color,border-color,color,opacity] duration-150 ease-soft disabled:cursor-not-allowed disabled:opacity-60 {confirmStyle}"
                    aria-busy={pending}
                    disabled={pending}
                    onclick={onconfirm}
                >
                    {pending ? 'Working…' : confirmLabel}
                </button>
            </div>
        </div>
    </div>
{/if}

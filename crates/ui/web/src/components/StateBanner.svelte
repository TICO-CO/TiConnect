<script lang="ts">
    import type { StateTone } from '../lib';
    import Icon from './Icon.svelte';

    interface Props {
        title: string;
        body?: string;
        tone?: StateTone;
        actionLabel?: string;
        onaction?: () => void;
        dismissLabel?: string;
        ondismiss?: () => void;
    }

    let {
        title,
        body = '',
        tone = 'neutral',
        actionLabel = '',
        onaction,
        dismissLabel = '',
        ondismiss,
    }: Props = $props();

    const styles = {
        neutral: {
            container: 'border-line bg-surface text-ink',
            label: 'text-ink-2',
            action: 'border-line-strong text-ink hover:bg-surface-2',
        },
        info: {
            container: 'border-info/55 bg-info/10 text-ink',
            label: 'text-info',
            action: 'border-info/70 text-info hover:bg-info/12',
        },
        success: {
            container: 'border-success/55 bg-success/10 text-ink',
            label: 'text-success',
            action: 'border-success/70 text-success hover:bg-success/12',
        },
        warning: {
            container: 'border-warning/55 bg-warning/10 text-ink',
            label: 'text-warning',
            action: 'border-warning/70 text-warning hover:bg-warning/12',
        },
        error: {
            container: 'border-error/55 bg-error/10 text-ink',
            label: 'text-error',
            action: 'border-error/70 text-error hover:bg-error/12',
        },
    } satisfies Record<StateTone, { container: string; label: string; action: string }>;
    const style = $derived(styles[tone]);
    const role = $derived(tone === 'warning' || tone === 'error' ? 'alert' : 'status');
</script>

<div class="flex items-start gap-3 rounded-md border px-4 py-3 {style.container}" {role}>
    <span
        class="mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border {style.label}"
    >
        <Icon
            name={tone === 'error' || tone === 'warning' ? 'alert' : 'check'}
            size={12}
            color="currentColor"
        />
    </span>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="text-sm leading-[1.4] font-bold {style.label}">{title}</span>
        {#if body}
            <span class="text-step-xs leading-[1.45] text-ink-2">{body}</span>
        {/if}
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
        {#if actionLabel && onaction}
            <button
                class="min-h-8 rounded-pill border px-3 text-xs font-bold transition-colors duration-150 ease-soft {style.action}"
                onclick={onaction}>{actionLabel}</button
            >
        {/if}
        {#if ondismiss}
            <button
                class="inline-flex size-8 items-center justify-center rounded-pill border border-transparent text-ink-2 hover:bg-surface-2 hover:text-ink"
                onclick={ondismiss}
                aria-label={dismissLabel || 'Dismiss'}
            >
                <Icon name="close" size={13} color="currentColor" />
            </button>
        {/if}
    </div>
</div>

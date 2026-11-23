<script lang="ts">
    import type { StateTone } from '../lib';

    interface Props {
        label: string;
        tone?: StateTone;
        dot?: boolean;
    }

    let { label, tone = 'neutral', dot = false }: Props = $props();

    const styles = {
        neutral: 'border-line-strong text-ink-2',
        info: 'border-info/60 bg-info/10 text-info',
        success: 'border-success/60 bg-success/10 text-success',
        warning: 'border-warning/60 bg-warning/10 text-warning',
        error: 'border-error/60 bg-error/10 text-error',
    } satisfies Record<StateTone, string>;
    const dotStyles = {
        neutral: 'bg-ink-3',
        info: 'bg-info',
        success: 'bg-success',
        warning: 'bg-warning',
        error: 'bg-error',
    } satisfies Record<StateTone, string>;
    const style = $derived(styles[tone]);
    const dotStyle = $derived(dotStyles[tone]);
</script>

<span
    class="inline-flex h-[22px] items-center gap-1.5 rounded-pill border px-2.5 text-xs leading-none font-bold whitespace-nowrap {style}"
>
    {#if dot}
        <span class="size-1.5 rounded-full {dotStyle}"></span>
    {/if}
    {label}
</span>

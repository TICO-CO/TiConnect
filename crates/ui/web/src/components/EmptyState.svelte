<script lang="ts">
    interface Props {
        title: string;
        body?: string;
        actionLabel?: string;
        onaction?: () => void;
        secondaryLabel?: string;
        onsecondary?: () => void;
    }

    let {
        title,
        body = '',
        actionLabel = '',
        onaction,
        secondaryLabel = '',
        onsecondary,
    }: Props = $props();
</script>

<div class="flex min-h-0 flex-1 flex-col items-center justify-center px-8 py-10 text-center">
    <div class="flex max-w-[360px] flex-col items-center gap-2">
        <span
            class="mb-1 flex size-12 items-center justify-center rounded-full border border-line bg-surface-2"
        >
            <span class="size-2 rounded-full bg-ink-3"></span>
        </span>
        <h2 class="m-0 font-display text-step-1 leading-[1.2] font-normal tracking-[-0.01em]">
            {title}
        </h2>
        {#if body}
            <p class="m-0 text-step-xs leading-[1.5] text-ink-2">{body}</p>
        {/if}
        {#if (actionLabel && onaction) || (secondaryLabel && onsecondary)}
            <div class="mt-2 flex items-center justify-center gap-2">
                {#if actionLabel && onaction}
                    <button
                        class="inline-flex min-h-9 items-center justify-center rounded-pill border border-accent bg-accent px-4 text-step-xs leading-none font-bold text-base hover:border-ink-2 hover:bg-ink-2"
                        onclick={onaction}>{actionLabel}</button
                    >
                {/if}
                {#if secondaryLabel && onsecondary}
                    <button
                        class="inline-flex min-h-9 items-center justify-center rounded-pill border border-line bg-transparent px-4 text-step-xs leading-none font-bold text-ink hover:border-line-strong hover:bg-surface"
                        onclick={onsecondary}>{secondaryLabel}</button
                    >
                {/if}
            </div>
        {/if}
    </div>
</div>

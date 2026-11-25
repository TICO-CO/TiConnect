<script lang="ts">
    interface Props {
        rows?: number;
        label?: string;
    }

    let { rows = 4, label = 'Loading' }: Props = $props();
    const rowIndexes = $derived(Array.from({ length: rows }, (_, index) => index));
</script>

<div class="flex flex-col gap-1" role="status" aria-label={label}>
    {#each rowIndexes as rowIndex (rowIndex)}
        <div class="flex h-16 items-center gap-2.5 px-6" aria-hidden="true">
            <div class="size-8 shrink-0 animate-pulse rounded-full bg-surface-2"></div>
            <div class="flex min-w-0 flex-1 flex-col gap-2">
                <div
                    class="h-3 animate-pulse rounded-sm bg-surface-2"
                    style={`width:${rowIndex % 3 === 0 ? '58%' : '76%'}`}
                ></div>
                <div
                    class="h-2.5 animate-pulse rounded-sm bg-surface-2"
                    style={`width:${rowIndex % 2 === 0 ? '84%' : '68%'}`}
                ></div>
            </div>
        </div>
    {/each}
    <span class="sr-only">{label}</span>
</div>

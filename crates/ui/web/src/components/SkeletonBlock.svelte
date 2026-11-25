<script lang="ts">
    interface Props {
        width?: string;
        height?: number;
        label?: string;
        radius?: string;
    }

    let { width = '100%', height = 16, label = '', radius = 'rounded-sm' }: Props = $props();
</script>

<div
    class="animate-pulse bg-surface-2 {radius}"
    style={`width:${width};height:${height}px`}
    role={label ? 'status' : undefined}
    aria-label={label || undefined}
    aria-hidden={label ? undefined : 'true'}
>
    {#if label}
        <span class="sr-only">{label}</span>
    {/if}
</div>

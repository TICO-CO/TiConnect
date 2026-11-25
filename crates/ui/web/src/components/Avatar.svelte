<script lang="ts">
    import type { User } from '../lib';
    import { initials, shortName } from '../lib';

    interface Props {
        user?: User | null;
        name?: string;
        size?: number;
        image?: string;
        showPresence?: boolean;
        selected?: boolean;
    }

    let {
        user = null,
        name = '',
        size = 32,
        image = '',
        showPresence = false,
        selected = false,
    }: Props = $props();
    const label = $derived(shortName(user ?? { display_name: name, id: name, fingerprint: name }));
    const connected = $derived(user?.connected ?? false);
</script>

<div
    class="relative inline-flex shrink-0 items-center justify-center rounded-full bg-surface-2 leading-none font-normal text-ink data-selected:bg-base data-selected:text-ink"
    data-selected={selected ? '' : undefined}
    style={`width:${size}px;height:${size}px;font-size:${Math.max(10, Math.round(size * 0.36))}px`}
    aria-hidden="true"
>
    {#if image}
        <img class="size-full rounded-full object-cover" src={image} alt="" />
    {:else}
        {initials(label)}
    {/if}
    {#if showPresence}
        <span
            class="absolute right-[-1px] bottom-[-1px] size-2 rounded-full border-2 border-rail bg-line-strong data-online:bg-success data-selected:border-ink"
            data-online={connected ? '' : undefined}
            data-selected={selected ? '' : undefined}
        ></span>
    {/if}
</div>

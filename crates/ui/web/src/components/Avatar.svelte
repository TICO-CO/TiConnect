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
    class="avatar"
    class:selected
    style={`width:${size}px;height:${size}px;font-size:${Math.max(10, Math.round(size * 0.36))}px`}
    aria-hidden="true"
>
    {#if image}
        <img src={image} alt="" />
    {:else}
        {initials(label)}
    {/if}
    {#if showPresence}
        <span class="presence" class:online={connected} class:selected></span>
    {/if}
</div>

<style>
    .avatar {
        position: relative;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        flex-shrink: 0;
        border-radius: 50%;
        background: var(--surface-2);
        color: var(--ink);
        font-weight: 400;
        line-height: 1;
    }

    .avatar.selected {
        background: var(--base);
        color: var(--ink);
    }

    .avatar img {
        width: 100%;
        height: 100%;
        border-radius: 50%;
        object-fit: cover;
    }

    .presence {
        position: absolute;
        right: -1px;
        bottom: -1px;
        width: 8px;
        height: 8px;
        border: 2px solid var(--rail);
        border-radius: 50%;
        background: var(--line-strong);
    }

    .presence.online {
        background: var(--success);
    }

    .presence.selected {
        border-color: var(--ink);
    }
</style>

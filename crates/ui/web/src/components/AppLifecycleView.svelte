<script lang="ts">
    import EmptyState from './EmptyState.svelte';
    import StateBanner from './StateBanner.svelte';

    interface Props {
        mode: 'booting' | 'offline';
        errorMessage?: string;
        reconnectAttempt?: number;
        onretry?: () => void;
    }

    let { mode, errorMessage = '', reconnectAttempt = 0, onretry }: Props = $props();
</script>

<section class="flex h-full min-w-0 flex-1 flex-col items-center justify-center bg-base px-8 py-10">
    <div class="flex w-full max-w-[460px] flex-col items-center gap-5">
        <a class="logo-mark size-12 shrink-0" href="#/chat" aria-label="TiConnect"></a>
        {#if mode === 'booting'}
            <div class="flex flex-col items-center gap-2 text-center" role="status">
                <h1
                    class="m-0 font-display text-step-2 leading-[1.15] font-normal tracking-[-0.01em]"
                >
                    Starting TiConnect
                </h1>
                <p class="m-0 text-step-xs leading-[1.5] text-ink-2">
                    Looking for the local daemon on this device.
                </p>
                <span class="mt-2 flex items-center gap-1.5" aria-hidden="true">
                    <span class="size-1.5 animate-pulse rounded-full bg-ink-3"></span>
                    <span
                        class="size-1.5 animate-pulse rounded-full bg-ink-3 [animation-delay:120ms]"
                    ></span>
                    <span
                        class="size-1.5 animate-pulse rounded-full bg-ink-3 [animation-delay:240ms]"
                    ></span>
                </span>
            </div>
        {:else}
            <EmptyState
                title="Cannot reach TiConnect"
                body="The local daemon is not responding. TiConnect keeps retrying in the background."
                actionLabel="Retry connection"
                onaction={onretry}
            />
            {#if reconnectAttempt > 0}
                <p class="m-0 text-center text-xs text-ink-3">
                    Retry attempt {reconnectAttempt}
                </p>
            {/if}
            {#if errorMessage}
                <div class="w-full">
                    <StateBanner
                        tone="error"
                        title="Connection failed"
                        body={errorMessage}
                        actionLabel="Retry"
                        onaction={onretry}
                    />
                </div>
            {/if}
        {/if}
    </div>
</section>

<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState } from '../lib';
    import { initials, sessionClock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let elapsed = $state(0);
    const user = $derived(app.userById(app.call?.user_id ?? app.selectedUserId));
    const active = $derived(app.call);
    const ringing = $derived(active?.state === 'ringing');
    const reconnecting = $derived(active?.state === 'reconnecting');
    const muted = $derived(active?.input_muted ?? false);
    const outputMuted = $derived(active?.output_muted ?? false);

    onMount(() => {
        const update = () => {
            if (app.call === null) {
                elapsed = 0;
                return;
            }
            const start = app.call.connected_at || app.call.started_at;
            elapsed = Math.max(0, Math.floor((Date.now() - start) / 1_000));
        };
        update();
        const timer = window.setInterval(update, 1_000);
        return () => window.clearInterval(timer);
    });
</script>

<section class="flex h-full min-w-0 flex-1 flex-col bg-base">
    <header class="flex h-[88px] shrink-0 items-center gap-3.5 bg-rail px-7">
        {#if user}
            <Avatar {user} image={app.avatarUrl(user.fingerprint)} size={40} />
            <div class="flex min-w-0 flex-1 flex-col gap-[3px]">
                <h1
                    class="m-0 font-display text-step-2 leading-[28px] font-normal tracking-[-0.01em]"
                >
                    {shortName(user)}
                </h1>
                <p class="m-0 flex items-center gap-1.5 text-[13px] text-ink-3">
                    <span
                        class="size-1.5 rounded-full border border-line-strong data-online:border-success data-online:bg-success"
                        data-online={user.connected ? '' : undefined}
                    ></span>
                    Voice call · {ringing ? 'Ringing' : reconnecting ? 'Reconnecting' : 'Encrypted'}
                </p>
            </div>
        {/if}
        <div class="ml-auto flex flex-col items-end gap-0.5">
            <strong class="text-[23px] leading-[26px] font-normal tabular-nums"
                >{sessionClock(elapsed)}</strong
            >
            <span
                class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                >Live · LAN</span
            >
        </div>
    </header>
    {#if active && user}
        <div class="flex flex-1 flex-col items-center justify-center gap-2">
            <div
                class="group relative flex size-[300px] items-center justify-center"
                data-state={reconnecting ? 'reconnecting' : undefined}
            >
                <span
                    class="absolute size-[300px] rounded-full border border-line-strong/80 group-data-[state=reconnecting]:border-warning/55"
                ></span>
                <span
                    class="absolute size-[238px] rounded-full border border-line-strong/80 group-data-[state=reconnecting]:border-warning/55"
                ></span>
                <span
                    class="flex size-[176px] items-center justify-center rounded-full bg-surface text-[47px] text-ink-2"
                    >{initials(shortName(user))}</span
                >
            </div>
            <h2
                class="mt-px mb-0 font-display text-[39px] leading-[44px] font-normal tracking-[-0.01em]"
            >
                {shortName(user)}
            </h2>
            <p class="m-0 text-step-4 leading-[38px] font-normal text-ink-2 tabular-nums">
                {sessionClock(elapsed)}
            </p>
            <p class="m-0 flex items-center gap-1.5 text-[13px] text-ink-3">
                <span
                    class="size-1.5 rounded-full border border-line-strong data-[state=online]:border-success data-[state=online]:bg-success data-[state=warning]:border-warning data-[state=warning]:bg-warning"
                    data-state={reconnecting ? 'warning' : 'online'}
                ></span>
                {ringing
                    ? 'Calling…'
                    : reconnecting
                      ? 'Reconnecting — audio resumes automatically'
                      : 'Connected · Opus · LAN'}
            </p>
            {#if muted}
                <span
                    class="mt-1 inline-flex items-center gap-1.5 rounded-pill bg-accent px-2.5 py-[5px] text-xs font-bold text-base"
                    ><Icon name="mute" size={13} color="var(--color-base)" /> Muted</span
                >
            {/if}
        </div>
        <div class="flex shrink-0 justify-center pb-9">
            <div
                class="flex items-stretch gap-4 rounded-lg border border-line bg-surface px-[18px] py-3.5"
            >
                <button
                    class="group flex min-w-16 flex-col items-center gap-1.5 border-0 bg-transparent text-xs text-ink-3 data-active:text-accent"
                    data-active={muted ? '' : undefined}
                    aria-pressed={muted}
                    onclick={() => void app.toggleInputMute()}
                >
                    <span
                        class="flex size-[38px] items-center justify-center rounded-full border border-line-strong group-hover:bg-surface-2 group-data-active:border-accent"
                        ><Icon name={muted ? 'mute' : 'mic'} size={18} /></span
                    >
                    <span>{muted ? 'Unmute' : 'Mute'}</span>
                </button>
                <button
                    class="group flex min-w-16 flex-col items-center gap-1.5 border-0 bg-transparent text-xs text-ink-3 data-active:text-accent"
                    data-active={outputMuted ? '' : undefined}
                    aria-pressed={outputMuted}
                    onclick={() => void app.toggleOutputMute()}
                >
                    <span
                        class="flex size-[38px] items-center justify-center rounded-full border border-line-strong group-hover:bg-surface-2 group-data-active:border-accent"
                        ><Icon name={outputMuted ? 'mute' : 'speaker'} size={18} /></span
                    >
                    <span>Output</span>
                </button>
                <button
                    class="group flex min-w-16 flex-col items-center gap-1.5 border-0 bg-transparent text-xs text-error"
                    onclick={() => void app.hangupCall()}
                >
                    <span
                        class="flex size-[38px] items-center justify-center rounded-full border border-error group-hover:bg-surface-2"
                        ><Icon name="call" size={18} color="var(--color-error)" /></span
                    >
                    <span>End call</span>
                </button>
            </div>
        </div>
    {:else}
        <div class="flex flex-1 flex-col items-center justify-center gap-[18px]">
            <div
                class="flex w-[min(560px,70%)] items-center justify-center gap-3 text-xs tracking-[0.06em] text-ink-3 uppercase"
            >
                <span class="h-px flex-1 bg-line"></span>Call ended<span class="h-px flex-1 bg-line"
                ></span>
            </div>
            <button
                class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                onclick={() => (user ? void app.startCall(user.id) : undefined)}>Call back</button
            >
        </div>
    {/if}
</section>

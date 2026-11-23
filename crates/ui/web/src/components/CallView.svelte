<script lang="ts">
    import { onMount } from 'svelte';

    import type { AppState, CallLog } from '../lib';
    import { clock, initials, sessionClock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import CallWaveform from './CallWaveform.svelte';
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
    const lastLog = $derived(
        user === null ? undefined : app.callLogs.find((log) => log.user_id === user.id),
    );
    const endedTitle = $derived(callEndTitle(lastLog));
    const endedDuration = $derived(lastLog?.duration_secs ?? 0);
    const endedConnected = $derived(endedDuration > 0);

    function callEndTitle(log: CallLog | undefined): string {
        switch (log?.reason) {
            case 'declined':
                return 'Call declined';
            case 'timeout':
                return 'Missed call';
            case 'error':
                return 'Call failed';
            case 'disconnected':
                return 'Connection lost';
            case 'hangup':
            case undefined:
                return 'Call ended';
        }
        return 'Call ended';
    }

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
                    {active
                        ? `Voice call · ${ringing ? 'Ringing' : reconnecting ? 'Reconnecting' : 'Encrypted'}`
                        : 'Call ended · Encrypted'}
                </p>
            </div>
        {/if}
        {#if active}
            <div class="ml-auto flex flex-col items-end gap-0.5">
                <strong class="text-[23px] leading-[26px] font-normal tabular-nums"
                    >{sessionClock(elapsed)}</strong
                >
                <span
                    class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Live · LAN</span
                >
            </div>
        {:else if lastLog}
            <div class="ml-auto flex flex-col items-end gap-0.5">
                <strong class="text-[23px] leading-[26px] font-normal tabular-nums"
                    >{clock(lastLog.ended_at)}</strong
                >
                <span
                    class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Ended · LAN</span
                >
            </div>
        {/if}
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
                <CallWaveform
                    level={app.callLevel}
                    tone={reconnecting ? 'warning' : 'normal'}
                    demo={app.demo}
                />
                <span
                    class="relative flex size-[176px] items-center justify-center rounded-full bg-surface text-[47px] text-ink-2"
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
                    class="group flex min-w-16 flex-col items-center gap-1.5 border-0 bg-transparent text-xs text-error disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={app.callActionPending}
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
        <div class="flex flex-1 flex-col items-center justify-center gap-[30px] pb-2">
            <div class="relative flex size-[300px] items-center justify-center">
                <span class="absolute size-[300px] rounded-full border border-line"></span>
                <svg
                    class="pointer-events-none absolute size-[300px]"
                    viewBox="0 0 300 300"
                    aria-hidden="true"
                >
                    <circle
                        cx="150"
                        cy="150"
                        r="139"
                        fill="none"
                        stroke="var(--color-accent)"
                        stroke-width="1.5"
                        stroke-linecap="round"
                        stroke-dasharray="86 787"
                        transform="rotate(-58 150 150)"
                        opacity="0.75"
                    />
                </svg>
                <span
                    class="relative flex size-[168px] items-center justify-center rounded-full border border-line-strong bg-surface font-display text-[48px] leading-[56px] text-ink-2"
                    >{user ? initials(shortName(user)) : '··'}</span
                >
            </div>
            <div class="flex flex-col items-center gap-3">
                <h2
                    class="m-0 font-display text-step-4 leading-[38px] font-normal tracking-[-0.01em]"
                >
                    {endedTitle}
                </h2>
                {#if endedConnected}
                    <p class="m-0 text-step-5 leading-[44px] font-normal text-ink-2 tabular-nums">
                        {sessionClock(endedDuration)}
                    </p>
                {/if}
                <p class="m-0 flex items-center gap-2 text-step-xs text-ink-3">
                    <span class="size-1.5 rounded-full bg-ink-3"></span>
                    {endedConnected ? 'Encrypted · Opus · LAN' : 'End-to-end encrypted'}
                </p>
                {#if lastLog}
                    <div class="flex items-center gap-3.5 pt-2.5">
                        <span class="text-xs font-bold tracking-[0.08em] text-ink-3 uppercase"
                            >Ended {clock(lastLog.ended_at)}</span
                        >
                        <span class="h-3 w-px bg-line-strong"></span>
                        <span class="text-step-xs text-ink-3"
                            >{lastLog.direction === 'incoming' ? 'Incoming' : 'Outgoing'} call</span
                        >
                    </div>
                {/if}
            </div>
        </div>
        {#if user}
            <div class="flex shrink-0 items-center justify-center gap-3 pb-12">
                <button
                    class="inline-flex min-h-12 items-center justify-center gap-2.5 rounded-pill border border-ink-2 bg-ink-2 px-[26px] text-[15px] leading-5 font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink hover:bg-ink"
                    onclick={() => void app.startCall(user.id)}
                >
                    <Icon name="call" size={16} color="var(--color-base)" />
                    Call back
                </button>
                <button
                    class="inline-flex min-h-12 items-center justify-center rounded-pill border border-line-strong bg-transparent px-6 text-[15px] leading-5 font-bold text-ink-2 transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-surface-2"
                    onclick={() => app.navigate({ kind: 'chat', id: user.id })}
                >
                    Message
                </button>
            </div>
        {/if}
    {/if}
</section>

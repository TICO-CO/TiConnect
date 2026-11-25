<script lang="ts">
    import { flip } from 'svelte/animate';
    import { cubicIn, cubicOut } from 'svelte/easing';
    import { fly } from 'svelte/transition';

    import type { AppState } from '../lib';
    import { clock, playerUrl, shortName, YOUTUBE_ORIGIN } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let playerFrame: HTMLIFrameElement | null = $state(null);
    let muted = $state(false);
    let now = $state(Date.now());
    const playerOrigin = typeof window === 'undefined' ? '' : window.location.origin;
    const reduceMotion =
        typeof window !== 'undefined' &&
        typeof window.matchMedia === 'function' &&
        window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    $effect(() => {
        const timer = window.setInterval(() => {
            now = Date.now();
        }, 1_000);
        return () => window.clearInterval(timer);
    });

    const incomingCall = $derived(
        app.call !== null && app.call.state === 'ringing' && app.call.direction === 'incoming'
            ? app.call
            : null,
    );
    const activeCall = $derived(
        app.call !== null && app.call.state !== 'ringing' && app.route.kind !== 'call'
            ? app.call
            : null,
    );
    const remoteRequest = $derived(
        app.remote !== null && app.remote.state === 'requested' && app.remote.role === 'host'
            ? app.remote
            : null,
    );
    const activeRemote = $derived(
        app.remote !== null && app.remote.state === 'active' && app.route.kind !== 'remote'
            ? app.remote
            : null,
    );
    const partyInvite = $derived(
        app.party !== null && app.party.state === 'ringing' && app.party.role === 'guest'
            ? app.party
            : null,
    );
    const activeParty = $derived(
        app.party !== null && app.party.state !== 'ringing' && app.route.kind !== 'watch-party'
            ? app.party
            : null,
    );
    const callSeconds = $derived(
        app.call === null
            ? 0
            : Math.max(
                  0,
                  Math.floor((now - (app.call.connected_at || app.call.started_at)) / 1_000),
              ),
    );
    const remoteSeconds = $derived(
        app.remote === null
            ? 0
            : Math.max(0, 300 - Math.floor((app.remote.expires_at - now) / 1_000)),
    );

    function post(command: string): void {
        playerFrame?.contentWindow?.postMessage(
            JSON.stringify({ event: 'command', func: command, args: [] }),
            YOUTUBE_ORIGIN,
        );
    }

    function openToast(userId: string, toastId: string): void {
        app.dismissToast(toastId);
        app.navigate({ kind: 'chat', id: userId });
    }

    function partyTitle(): string {
        return app.demo ? 'How we built TiConnect for the LAN' : 'YouTube watch party';
    }
</script>

<div
    class="pointer-events-none fixed top-6 right-6 z-30 flex w-[360px] flex-col gap-2"
    aria-live="polite"
>
    {#if incomingCall}
        {@const caller = app.userById(incomingCall.user_id)}
        <article
            class="pointer-events-auto flex flex-col rounded-lg border border-line-strong bg-surface px-4 py-3.5 shadow-soft"
        >
            <div class="flex min-h-8 items-center gap-2">
                <Avatar
                    user={caller}
                    image={caller ? app.avatarUrl(caller.fingerprint) : ''}
                    name={app.userName(incomingCall.user_id)}
                    size={32}
                />
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Incoming call</span
                >
                <span class="text-xs text-ink-3 tabular-nums">{clock(incomingCall.started_at)}</span
                >
            </div>
            <h2 class="mt-2 mb-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]">
                {app.userName(incomingCall.user_id)}
            </h2>
            <p class="mt-0.5 mb-0 text-[13px] text-ink-2">Voice call · LAN</p>
            <div class="mt-3 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-error/12 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={app.callActionPending}
                    onclick={() => void app.declineCall()}
                    >{app.callActionPending ? 'Declining…' : 'Decline'}</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-3.5 text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2 disabled:cursor-not-allowed disabled:opacity-60"
                    aria-busy={app.callActionPending}
                    disabled={app.callActionPending}
                    onclick={() => void app.acceptCall()}
                    >{app.callActionPending ? 'Accepting…' : 'Accept'}</button
                >
            </div>
        </article>
    {/if}
    {#if remoteRequest}
        {@const requester = app.userById(remoteRequest.user_id)}
        <article
            class="pointer-events-auto flex flex-col rounded-lg border border-line-strong bg-surface px-4 py-3.5 shadow-soft"
        >
            <div class="flex min-h-8 items-center gap-2">
                <Avatar
                    user={requester}
                    image={requester ? app.avatarUrl(requester.fingerprint) : ''}
                    name={app.userName(remoteRequest.user_id)}
                    size={32}
                />
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Remote desktop</span
                >
                <span class="text-xs text-ink-3 tabular-nums">Now</span>
            </div>
            <h2 class="mt-2 mb-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]">
                {app.userName(remoteRequest.user_id)}
            </h2>
            <p class="mt-0.5 mb-0 text-[13px] text-ink-2">Wants to view and control this screen</p>
            <div class="mt-3 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-error/12"
                    onclick={() => void app.denyRemote()}>Decline</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-3.5 text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                    onclick={() => void app.acceptRemote()}>Accept</button
                >
            </div>
        </article>
    {/if}
    {#if partyInvite}
        {@const partyUser = app.userById(partyInvite.host_id)}
        <article
            class="pointer-events-auto flex flex-col rounded-lg border border-line-strong bg-surface px-4 py-3.5 shadow-soft"
        >
            <div class="flex min-h-8 items-center gap-2">
                <Avatar
                    user={partyUser}
                    image={partyUser ? app.avatarUrl(partyUser.fingerprint) : ''}
                    name={app.userName(partyInvite.host_id)}
                    size={32}
                />
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Watch party invite</span
                >
                <span class="text-xs text-ink-3 tabular-nums">{clock(partyInvite.updated_at)}</span>
            </div>
            <h2 class="mt-2 mb-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]">
                {app.userName(partyInvite.host_id)}
            </h2>
            <p class="mt-0.5 mb-0 text-[13px] text-ink-2">Watch party · {partyTitle()}</p>
            <div class="mt-3 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-error/12"
                    onclick={() => void app.declineParty()}>Decline</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-3.5 text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                    onclick={() => void app.acceptParty()}>Join</button
                >
            </div>
        </article>
    {/if}
    {#if activeCall}
        {@const callUser = app.userById(activeCall.user_id)}
        <article
            class="group pointer-events-auto flex flex-col rounded-lg border border-line-strong bg-surface px-4 py-3.5 shadow-soft"
            data-active=""
        >
            <div class="flex min-h-8 items-center gap-2">
                <span class="size-[7px] rounded-full bg-accent group-data-active:bg-success"></span>
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >{activeCall.state === 'connecting' ? 'Calling' : 'In call'}</span
                >
                <span class="text-xs text-ink-3 tabular-nums"
                    >{new Date(callSeconds * 1000).toISOString().slice(11, 19)}</span
                >
            </div>
            <h2 class="mt-2 mb-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]">
                {app.userName(activeCall.user_id)}
            </h2>
            <div class="mt-2 flex items-center gap-2 text-xs text-ink-3">
                <Avatar
                    user={callUser}
                    image={callUser ? app.avatarUrl(callUser.fingerprint) : ''}
                    name={app.userName(activeCall.user_id)}
                    size={24}
                />
                <span>{app.userName(activeCall.user_id)}</span>
            </div>
            <div class="mt-3 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    aria-pressed={activeCall.input_muted}
                    onclick={() => void app.toggleInputMute()}
                    >{activeCall.input_muted ? 'Unmute' : 'Mute'}
                    <Icon name="mute" size={12} /></button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-error/12 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={app.callActionPending}
                    onclick={() => void app.hangupCall()}>End</button
                >
            </div>
        </article>
    {/if}
    {#if activeRemote}
        <article
            class="group pointer-events-auto flex flex-col rounded-lg border border-line-strong bg-surface px-4 py-3.5 shadow-soft"
            data-active=""
        >
            <div class="flex min-h-8 items-center gap-2">
                <span class="size-[7px] rounded-full bg-accent group-data-active:bg-success"></span>
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Sharing screen</span
                >
                <span class="text-xs text-ink-3 tabular-nums"
                    >{new Date(remoteSeconds * 1000).toISOString().slice(11, 19)}</span
                >
            </div>
            <h2 class="mt-2 mb-0 font-display text-[19px] leading-6 font-normal tracking-[-0.01em]">
                {app.userName(activeRemote.user_id)}
            </h2>
            <p class="mt-0.5 mb-0 text-[13px] text-ink-2">Indicator stays visible while sharing.</p>
            <div class="mt-3 flex justify-end gap-2">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-error/12"
                    onclick={() => void app.endRemote()}>End session</button
                >
            </div>
        </article>
    {/if}
    {#if activeParty}
        <button
            class="pointer-events-auto flex h-[46px] w-full items-center gap-2.5 rounded-pill border border-line bg-surface-2 pr-2 pl-[14px] text-[13px] text-ink shadow-soft"
            onclick={() => app.navigate({ kind: 'watch-party' })}
        >
            <span
                class="flex size-[30px] items-center justify-center rounded-full border border-line-strong"
                ><Icon name="play" size={12} color="var(--color-ink-2)" /></span
            >
            <span>Watch party · {activeParty.participants.length} watching</span>
            <span
                class="ml-auto rounded-pill border border-line-strong px-2.5 py-1 text-xs text-ink-2"
                >Open</span
            >
        </button>
    {/if}
</div>

<div
    class="pointer-events-none fixed right-6 z-30 flex w-[420px] flex-col gap-2 transition-[bottom] duration-200 ease-soft"
    style={`bottom:${app.player && app.route.kind !== 'watch-party' ? 442 : 24}px`}
    aria-live="polite"
>
    {#each app.toasts as toast (toast.id)}
        <div
            class="pointer-events-auto flex w-full items-center gap-2.5 rounded-lg border border-line-strong bg-surface-2 px-3.5 py-3 text-left text-ink shadow-soft"
            role="button"
            tabindex="0"
            in:fly={{
                y: reduceMotion ? 0 : 16,
                duration: reduceMotion ? 0 : 180,
                easing: cubicOut,
            }}
            out:fly={{ y: reduceMotion ? 0 : 8, duration: reduceMotion ? 0 : 140, easing: cubicIn }}
            animate:flip={{ duration: reduceMotion ? 0 : 160, easing: cubicOut }}
            onclick={() => openToast(toast.userId, toast.id)}
            onkeydown={(event) => {
                if (event.key === 'Enter') {
                    openToast(toast.userId, toast.id);
                }
            }}
        >
            <Avatar name={toast.name} size={32} />
            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                <span class="flex items-center justify-between gap-2.5">
                    <span class="text-xs font-bold tracking-[0.06em] text-ink uppercase"
                        >{toast.name}</span
                    >
                    <span class="text-xs text-ink-3 tabular-nums">{clock(toast.sentAt)}</span>
                </span>
                <span class="truncate text-sm leading-[1.4] text-ink">{toast.body}</span>
            </span>
            <button
                class="flex size-6 shrink-0 items-center justify-center rounded-full hover:bg-surface"
                onclick={(event) => {
                    event.stopPropagation();
                    app.dismissToast(toast.id);
                }}
                aria-label="Dismiss"
            >
                <Icon name="close" size={13} color="var(--color-ink-3)" />
            </button>
        </div>
    {/each}
</div>

{#if app.player && app.route.kind !== 'watch-party'}
    <div class="pointer-events-auto fixed right-6 bottom-6 z-[31] w-[480px]">
        {#if app.messages.length > 0}
            <div
                class="mb-2 flex flex-col gap-0.5 rounded-md border border-line bg-surface-2 px-3 py-2"
            >
                <span
                    class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                    >Up next · 2</span
                >
                <strong class="text-xs">LAN discovery in 90 seconds</strong>
            </div>
        {/if}
        <div class="flex flex-col rounded-lg border border-line-strong bg-surface p-3 shadow-soft">
            <div class="flex items-center gap-2 pb-2.5">
                <span
                    class="flex-1 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-2 uppercase"
                    >YouTube</span
                >
                <span
                    class="flex items-center gap-2 text-xs tracking-[0.04em] text-ink-3 uppercase"
                >
                    Ignore incoming
                    <button
                        class="small toggle"
                        aria-label="Ignore incoming videos"
                        aria-pressed={app.preferences.ignoreIncoming}
                        onclick={() => app.toggleIgnoreIncoming()}
                    ></button>
                </span>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => app.closePlayer()}
                    aria-label="Close player"
                >
                    <Icon name="close" size={14} />
                </button>
            </div>
            <div class="flex aspect-video overflow-hidden rounded-sm bg-surface-2">
                <iframe
                    class="size-full border-0"
                    bind:this={playerFrame}
                    src={playerUrl(app.player.videoId, playerOrigin, true)}
                    title={app.player.title}
                    allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
                    allowfullscreen
                ></iframe>
            </div>
            <div class="flex flex-col gap-0.5 py-2.5">
                <strong class="text-[15px]">{app.player.title}</strong>
                <span class="text-xs tracking-[0.04em] text-ink-3 uppercase"
                    >{app.player.channel} · LAN</span
                >
            </div>
            <div class="flex items-center gap-2">
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => post('playVideo')}
                    aria-label="Play"
                >
                    <Icon name="play" size={14} />
                </button>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => post('pauseVideo')}
                    aria-label="Pause"
                >
                    <Icon name="pause" size={14} />
                </button>
                <span class="text-xs text-ink-3 tabular-nums">06:12</span>
                <span class="h-[3px] flex-1 rounded-pill bg-line"
                    ><span class="block h-full w-[34%] rounded-pill bg-accent"></span></span
                >
                <span class="text-xs text-ink-3 tabular-nums">18:42</span>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    onclick={() => {
                        muted = !muted;
                        post(muted ? 'mute' : 'unMute');
                    }}
                    aria-label={muted ? 'Unmute' : 'Mute'}
                >
                    <Icon name={muted ? 'mute' : 'speaker'} size={14} />
                </button>
                <button
                    class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
                    aria-label="Fullscreen"
                >
                    <Icon name="fullscreen" size={14} />
                </button>
            </div>
        </div>
    </div>
{/if}

<script lang="ts">
    import type { AppState } from '../lib';
    import { fingerprintLabel, sessionClock, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';

    interface Props {
        app: AppState;
    }

    let { app }: Props = $props();
    let inputAllowed = $state(true);
    const session = $derived(app.remote);
    const user = $derived(app.userById(session?.user_id ?? app.selectedUserId));
    const active = $derived(session?.state === 'active');
    const waiting = $derived(session === null || session.state === 'requested');
    const host = $derived(session?.role === 'host');
    const elapsed = $derived(
        app.demo ? 221 : Math.max(0, (Date.now() - (session?.expires_at ?? 0)) / -1000),
    );
</script>

<section class="flex h-full min-w-0 flex-1 flex-col overflow-hidden bg-base">
    <header class="flex h-[88px] shrink-0 items-center gap-3.5 bg-rail px-7">
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
            onclick={() => app.backToChat()}
            aria-label="Back"
        >
            <Icon name="back" size={16} />
        </button>
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <h1 class="m-0 font-display text-step-3 leading-[34px] font-normal tracking-[-0.01em]">
                {#if user}
                    {host ? `Sharing with ${shortName(user)}` : `${shortName(user)} is sharing`}
                {:else}
                    Remote desktop
                {/if}
            </h1>
            <p class="m-0 text-[13px] text-ink-3">Monitor 1 · Encrypted · LAN</p>
        </div>
        <span
            class="flex h-[34px] items-center gap-1.5 rounded-pill border border-line px-3.5 text-xs tracking-[0.06em] text-ink-2 uppercase"
            ><span class="size-[7px] rounded-full bg-error"></span>Live · {sessionClock(
                Math.max(0, elapsed),
            )}</span
        >
    </header>
    {#if waiting}
        <div class="flex flex-1 flex-col items-center justify-center gap-2.5 text-center">
            <span class="flex size-[88px] items-center justify-center rounded-full bg-surface"
                ><Icon name="remote" size={28} color="var(--color-ink-2)" /></span
            >
            <h2
                class="mt-2 mb-0 font-display text-step-2 leading-[1.15] font-normal tracking-[-0.01em]"
            >
                {host ? 'Waiting for the request' : 'Waiting for consent'}
            </h2>
            <p class="m-0 text-step-xs text-ink-3">
                {host
                    ? 'The request stays open until you accept or it expires.'
                    : 'Capture starts only after the other side accepts.'}
            </p>
            {#if host && session && session.fingerprint}
                <p class="m-0 font-mono text-[11px] text-ink-2" title={session.fingerprint}>
                    Verify identity {fingerprintLabel(session.fingerprint)} before accepting.
                </p>
            {/if}
            <div class="mt-2 flex gap-2.5">
                <button
                    class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                    onclick={() => (host ? void app.denyRemote() : void app.endRemote())}
                >
                    Cancel request
                </button>
                {#if host && session}
                    <button
                        class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2"
                        onclick={() => void app.acceptRemote()}>Accept</button
                    >
                {/if}
            </div>
        </div>
    {:else}
        <div class="flex min-h-0 flex-1 gap-5 overflow-hidden px-7 py-5">
            <div class="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto">
                <div class="flex flex-col gap-2.5 rounded-md border border-line bg-surface p-3">
                    <div class="flex items-center justify-between gap-3">
                        <span class="text-xs tracking-[0.06em] text-ink uppercase"
                            ><span class="mr-1.5 inline-block size-[7px] rounded-full bg-error"
                            ></span>Sharing screen · {user ? shortName(user) : ''}</span
                        >
                        <span class="text-xs text-ink-3">LAN · 1080p · 24 fps</span>
                    </div>
                    <div
                        class="flex aspect-video min-h-[300px] items-stretch overflow-hidden rounded-sm bg-backdrop"
                    >
                        <div class="flex flex-1 flex-col gap-2.5 p-3 text-[11px] text-ink-3">
                            <div class="flex items-center gap-[5px] border-b border-line pb-2">
                                <span class="size-1.5 rounded-full bg-line-strong"></span>
                                <span class="size-1.5 rounded-full bg-line-strong"></span>
                                <span class="size-1.5 rounded-full bg-line-strong"></span>
                                <span class="ml-2 text-ink-3">ticonnect — release-notes.md</span>
                            </div>
                            <div class="grid min-h-0 flex-1 grid-cols-[140px_1fr_150px] gap-[18px]">
                                <div class="flex flex-col gap-[7px] border-r border-line pr-3">
                                    <span
                                        class="mt-1.5 font-text text-[9px] leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                                        >Explorer</span
                                    >
                                    <strong class="text-[11px] font-bold text-ink-2"
                                        >TICONNECT</strong
                                    >
                                    <span>crates/</span><span>docs/</span><span
                                        class="rounded-[4px] bg-surface-2 px-1.5 py-0.5 text-ink"
                                        >release-notes.md</span
                                    >
                                    <span>ux-review.md</span><span>CHANGELOG.md</span>
                                </div>
                                <div>
                                    <h3
                                        class="mt-0 mb-1 font-display text-[23px] leading-[1.15] font-normal tracking-[-0.01em] text-ink"
                                    >
                                        TiConnect 0.9.2
                                    </h3>
                                    <p class="mt-0 mb-4 text-ink-3">
                                        Quiet Grove release · LAN-only
                                    </p>
                                    <ul
                                        class="m-0 flex list-disc flex-col gap-[7px] pl-[18px] text-ink-2"
                                    >
                                        <li>Messages, calls, and sharing stay on the LAN</li>
                                        <li>No accounts, no relays, no cloud storage</li>
                                        <li>Remote desktop requires fresh host consent</li>
                                        <li>48 tests passing · 0 warnings</li>
                                    </ul>
                                </div>
                                <div class="flex flex-col gap-1 border-l border-line pl-3">
                                    <span
                                        class="mt-1.5 font-text text-[9px] leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                                        >Build</span
                                    >
                                    <strong class="text-[11px] font-bold text-success!"
                                        >Passing</strong
                                    >
                                    <span>cargo test · workspace</span>
                                    <span
                                        class="mt-1.5 font-text text-[9px] leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                                        >Branch</span
                                    >
                                    <strong class="text-[11px] font-bold text-ink-2">main</strong>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <p class="m-0 text-xs text-ink-3">
                    <span class="mr-1.5 inline-block size-[7px] rounded-full bg-error"
                    ></span>Monitor 1 · 2560 × 1440 · indicator stays visible while sharing
                </p>
                {#if host}
                    <section class="rounded-md border border-line bg-surface p-4">
                        <h2
                            class="mt-0 mb-2.5 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                        >
                            This session
                        </h2>
                        <div
                            class="grid grid-cols-[70px_1fr_110px] gap-2.5 py-[7px] text-[13px] text-ink-2"
                        >
                            <span>09:38</span><span>Sharing started</span><span
                                class="text-right text-ink-3">Monitor 1</span
                            >
                        </div>
                        <div
                            class="grid grid-cols-[70px_1fr_110px] gap-2.5 py-[7px] text-[13px] text-ink-2"
                        >
                            <span>09:40</span><span>{user ? shortName(user) : 'Guest'} joined</span
                            ><span class="text-right text-ink-3">Encrypted</span>
                        </div>
                        <div
                            class="grid grid-cols-[70px_1fr_110px] gap-2.5 py-[7px] text-[13px] text-ink-2"
                        >
                            <span>09:41</span><span>Keyboard control enabled</span><span
                                class="text-right text-ink-3"
                                >{inputAllowed ? 'Allowed' : 'Blocked'}</span
                            >
                        </div>
                    </section>
                {/if}
            </div>
            <aside class="w-[300px] shrink-0 overflow-y-auto">
                {#if host}
                    <section class="rounded-md border border-line bg-surface p-[18px]">
                        <div class="flex items-center justify-between text-xs text-ink-3">
                            <h2
                                class="m-0 font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                            >
                                Sharing
                            </h2>
                            <span>{sessionClock(Math.max(0, elapsed))}</span>
                        </div>
                        <p class="mt-2.5 mb-3.5 text-[19px] text-ink">
                            <span class="mr-1.5 inline-block size-[7px] rounded-full bg-error"
                            ></span>Sharing screen
                        </p>
                        <div class="flex items-center gap-2.5 border-b border-line py-2.5">
                            {#if user}<Avatar
                                    {user}
                                    image={app.avatarUrl(user.fingerprint)}
                                    size={32}
                                />{/if}
                            <div class="flex flex-col gap-0.5">
                                <strong class="text-sm"
                                    >{user ? shortName(user) : 'Remote viewer'}</strong
                                ><span class="text-xs text-ink-3">Encrypted · LAN</span>
                            </div>
                        </div>
                        <div
                            class="flex items-center justify-between gap-4 border-b border-line py-3.5"
                        >
                            <div class="flex flex-col gap-0.5">
                                <strong class="text-[13px]">Allow pointer and keyboard</strong><span
                                    class="text-xs text-ink-3">Control stops when sharing ends</span
                                >
                            </div>
                            <button
                                class="toggle"
                                aria-label="Allow pointer and keyboard"
                                aria-pressed={inputAllowed}
                                onclick={() => (inputAllowed = !inputAllowed)}
                            ></button>
                        </div>
                        <p class="my-3.5 text-[13px] leading-normal text-ink-3">
                            Stop sharing ends capture and input immediately. The indicator
                            disappears only after teardown.
                        </p>
                        <button
                            class="inline-flex min-h-9 w-full items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                            onclick={() => void app.endRemote()}>Stop sharing</button
                        >
                    </section>
                {:else}
                    <section class="rounded-md border border-line bg-surface p-[18px]">
                        <h2
                            class="font-text text-xs leading-4 font-bold tracking-[0.06em] text-ink-3 uppercase"
                        >
                            Session
                        </h2>
                        <div class="flex items-center gap-2.5 border-b border-line py-2.5">
                            {#if user}<Avatar
                                    {user}
                                    image={app.avatarUrl(user.fingerprint)}
                                    size={32}
                                />{/if}
                            <div class="flex flex-col gap-0.5">
                                <strong class="text-sm">{user ? shortName(user) : 'Host'}</strong
                                ><span class="text-xs text-ink-3">Encrypted · LAN</span>
                            </div>
                        </div>
                        <p class="my-3.5 text-[13px] leading-normal text-ink-3">
                            You are viewing this screen. Input is controlled by the host.
                        </p>
                        <button
                            class="inline-flex min-h-9 w-full items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-[18px] text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                            onclick={() => void app.endRemote()}>Leave session</button
                        >
                    </section>
                {/if}
            </aside>
        </div>
        {#if !host}
            <div class="flex shrink-0 justify-center gap-2 pb-5">
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    ><Icon name="pointer" size={14} />Pointer</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    ><Icon name="keyboard" size={14} />Keyboard</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-3.5 text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface"
                    ><Icon name="fullscreen" size={14} />Fullscreen</button
                >
                <button
                    class="inline-flex min-h-8 items-center justify-center gap-1.5 rounded-pill border border-error bg-transparent px-3.5 text-step-xs leading-none font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                    onclick={() => void app.endRemote()}>End session</button
                >
            </div>
        {/if}
    {/if}
</section>

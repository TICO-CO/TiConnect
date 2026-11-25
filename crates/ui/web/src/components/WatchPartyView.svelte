<script lang="ts">
    import { onMount, untrack } from 'svelte';
    import { SvelteMap } from 'svelte/reactivity';

    import type { AppState, User, YoutubePreview } from '../lib';
    import { firstVideoId, formatPlayerTime, shortName } from '../lib';
    import Avatar from './Avatar.svelte';
    import Icon from './Icon.svelte';
    import StateBanner from './StateBanner.svelte';
    import YoutubePlayer from './YoutubePlayer.svelte';

    interface Props {
        app: AppState;
    }

    interface Participant {
        id: string;
        name: string;
        avatar: string;
        host: boolean;
        self: boolean;
        connected: boolean;
    }

    interface PreviewState {
        status: 'idle' | 'loading' | 'ready' | 'error';
        data: YoutubePreview | null;
    }

    let { app }: Props = $props();
    let videoInput = $state('');
    let selected = $state<string[]>(untrack(() => app.takeWatchPartySelection()));
    let inviteSelected = $state<string[]>([]);
    let inviteOpen = $state(false);
    let query = $state('');
    let inviteQuery = $state('');
    let peopleSearch: HTMLInputElement | null = $state(null);
    let inviteSearch: HTMLInputElement | null = $state(null);
    let preview = $state<PreviewState>({ status: 'idle', data: null });
    let previewRequest = 0;
    let thumbnailFailed = $state(false);
    let thumbnailFallback = $state(false);
    const previewCache = new SvelteMap<string, YoutubePreview>();

    const party = $derived(app.party);
    const videoId = $derived(firstVideoId(videoInput));
    const selfId = $derived(app.bootstrap?.identity.node_id ?? '');
    const onlineUsers = $derived(app.users.filter((user) => user.id !== selfId));
    const host = $derived(party === null ? null : app.userById(party.host_id));
    const isHost = $derived(party !== null && party.host_id === selfId);
    const participants = $derived(
        party === null ? [] : party.participants.map((id) => participant(id)),
    );
    const invited = $derived(
        party === null
            ? []
            : party.invited
                  .filter((id) => !party.participants.includes(id))
                  .map((id) => participant(id)),
    );
    const invitable = $derived(
        party === null
            ? []
            : app.users.filter(
                  (user) =>
                      user.id !== selfId &&
                      user.connected &&
                      !party.participants.includes(user.id) &&
                      !party.invited.includes(user.id),
              ),
    );
    const waiting = $derived(party !== null && party.state === 'ringing' && party.role === 'host');
    const onlineCount = $derived(onlineUsers.filter((user) => user.connected).length);
    const visibleUsers = $derived(onlineUsers.filter((user) => matches(user, query)));
    const visibleInvitable = $derived(invitable.filter((user) => matches(user, inviteQuery)));
    const guestInvite = $derived(
        party !== null && party.state === 'ringing' && party.role === 'guest',
    );
    const controlling = $derived(party?.actor ?? '');
    const playerTitle = $derived(party === null ? '' : `YouTube video ${party.video_id}`);
    const remoteThumbnail = $derived(
        preview.data?.thumbnail_url ??
            (videoId === null ? '' : `https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`),
    );
    const previewThumbnail = $derived(
        preview.data?.thumbnail_data && !thumbnailFallback
            ? preview.data.thumbnail_data
            : remoteThumbnail,
    );
    const hasThumbnail = $derived(previewThumbnail !== '' && !thumbnailFailed);
    const previewTitle = $derived(
        preview.data?.title ?? (videoId === null ? '' : `YouTube video ${videoId}`),
    );
    const previewChannel = $derived(preview.data?.channel ?? 'YouTube');
    const previewDescription = $derived(preview.data?.description ?? '');
    const previewDuration = $derived(preview.data?.duration_seconds ?? 0);

    $effect(() => {
        const id = videoId;
        const request = ++previewRequest;
        thumbnailFailed = false;
        thumbnailFallback = false;
        if (id === null) {
            preview = { status: 'idle', data: null };
            return;
        }
        const cached = previewCache.get(id);
        if (cached !== undefined) {
            preview = { status: 'ready', data: cached };
            app.prefetchYouTubeStream(id);
            return;
        }
        preview = { status: 'loading', data: null };
        const timer = window.setTimeout(() => {
            app.prefetchYouTubeStream(id);
            void app.youtubePreview(id).then((data) => {
                if (request !== previewRequest) {
                    return;
                }
                if (data === null) {
                    preview = { status: 'error', data: null };
                    return;
                }
                thumbnailFailed = false;
                thumbnailFallback = false;
                previewCache.set(id, data);
                preview = { status: 'ready', data };
            });
        }, 400);
        return () => window.clearTimeout(timer);
    });

    function thumbnailError(): void {
        if (preview.data?.thumbnail_data && !thumbnailFallback) {
            thumbnailFallback = true;
            return;
        }
        thumbnailFailed = true;
    }

    function participant(id: string): Participant {
        const user = app.userById(id);
        return {
            id,
            name: app.userName(id),
            avatar: app.participantAvatar(id),
            host: party?.host_id === id,
            self: id === selfId,
            connected: id === selfId || (user?.connected ?? false),
        };
    }

    function matches(user: User, term: string): boolean {
        const needle = term.trim().toLowerCase();
        return needle === '' || shortName(user).toLowerCase().includes(needle);
    }

    onMount(() => {
        const onKey = (event: KeyboardEvent) => {
            const target = event.target as HTMLElement | null;
            const editing =
                target?.tagName === 'INPUT' ||
                target?.tagName === 'TEXTAREA' ||
                target?.isContentEditable;
            const targetInput = inviteOpen ? inviteSearch : peopleSearch;
            if (event.key !== '/' || editing || targetInput === null) {
                return;
            }
            event.preventDefault();
            targetInput.focus();
        };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    });

    function toggleSelected(id: string): void {
        selected = selected.includes(id)
            ? selected.filter((item) => item !== id)
            : [...selected, id];
    }

    function toggleInvite(id: string): void {
        inviteSelected = inviteSelected.includes(id)
            ? inviteSelected.filter((item) => item !== id)
            : [...inviteSelected, id];
    }

    function start(): void {
        if (videoId === null) {
            app.notice = 'Paste a valid YouTube link first.';
            return;
        }
        if (selected.length === 0) {
            app.notice = 'Select at least one person.';
            return;
        }
        void app.startParty(videoId, selected);
    }

    function sendVideo(): void {
        if (videoId === null) {
            app.notice = 'Paste a valid YouTube link first.';
            return;
        }
        void app.sendVideoToUsers(selected, videoId);
    }

    function invite(): void {
        if (inviteSelected.length === 0) {
            return;
        }
        void app.inviteParty(inviteSelected);
        inviteSelected = [];
        inviteQuery = '';
        inviteOpen = false;
    }

    function leave(): void {
        if (party === null) {
            return;
        }
        if (party.role === 'host') {
            void app.endParty();
        } else {
            void app.leaveParty();
        }
    }

    function partyOutcomeTitle(reason: string): string {
        switch (reason) {
            case 'declined':
                return 'Watch party declined';
            case 'left':
                return 'You left the watch party';
            case 'host ended':
                return 'Host ended the watch party';
            case 'host disconnected':
                return 'Host disconnected';
            default:
                return 'Watch party ended';
        }
    }

    function partyOutcomeBody(reason: string, hostId: string): string {
        const hostName = app.userName(hostId);
        switch (reason) {
            case 'declined':
                return `${hostName} declined the invitation. No one joined the party.`;
            case 'left':
                return 'Playback and sync have stopped on this device.';
            case 'host ended':
                return `${hostName} ended the party for everyone.`;
            case 'host disconnected':
                return `${hostName} disconnected, so the party closed.`;
            default:
                return 'Playback and sync have stopped on this device.';
        }
    }
</script>

<section class="flex h-full min-w-0 flex-1 flex-col overflow-hidden bg-base">
    <div class="flex min-h-0 flex-1 flex-col px-8 pt-5 pb-7">
        <button
            class="inline-flex size-8 shrink-0 items-center justify-center rounded-pill border border-transparent bg-transparent text-ink hover:bg-surface"
            onclick={() => app.backToChat()}
            aria-label="Back"
        >
            <Icon name="back" size={16} />
        </button>

        {#if party === null && app.partyOutcome}
            <div class="mt-3">
                <StateBanner
                    tone="warning"
                    title={partyOutcomeTitle(app.partyOutcome.reason)}
                    body={partyOutcomeBody(app.partyOutcome.reason, app.partyOutcome.hostId)}
                    dismissLabel="Dismiss party result"
                    ondismiss={() => (app.partyOutcome = null)}
                />
            </div>
        {/if}

        <div class="mt-[18px] flex min-h-0 flex-1 gap-5">
            {#if party === null}
                <div
                    class="flex min-w-0 flex-1 flex-col rounded-md border border-line bg-surface p-3"
                >
                    <div class="flex flex-col gap-0.5 px-1 pb-2.5">
                        <h1 class="m-0 leading-5 font-bold text-base text-ink">
                            Start a watch party
                        </h1>
                        <span
                            class="font-text text-xs leading-4 tracking-[0.06em] text-ink-3 uppercase"
                            >Paste a YouTube link, then pick people</span
                        >
                    </div>
                    <div
                        class="flex min-h-[300px] flex-1 flex-col items-center justify-center gap-4 rounded-sm border border-line/60 bg-surface-2 p-8"
                    >
                        {#if videoId !== null}
                            <div
                                class="flex w-full max-w-[520px] flex-col gap-3"
                                aria-busy={preview.status === 'loading'}
                            >
                                <div
                                    class="relative aspect-video w-full overflow-hidden rounded-sm bg-base"
                                >
                                    {#if hasThumbnail}
                                        <img
                                            class="h-full w-full object-cover"
                                            src={previewThumbnail}
                                            alt="Video thumbnail"
                                            onerror={thumbnailError}
                                        />
                                        <span
                                            class="pointer-events-none absolute inset-0 flex items-center justify-center"
                                            aria-hidden="true"
                                        >
                                            <span
                                                class="flex size-16 items-center justify-center rounded-pill bg-accent pl-0.5"
                                            >
                                                <Icon
                                                    name="play"
                                                    size={24}
                                                    color="var(--color-base)"
                                                />
                                            </span>
                                        </span>
                                        {#if previewDuration > 0}
                                            <span
                                                class="absolute right-3 bottom-3 rounded-pill bg-base/80 px-2 py-1 text-xs leading-4 text-ink tabular-nums"
                                                >{formatPlayerTime(previewDuration)}</span
                                            >
                                        {/if}
                                    {:else}
                                        <div
                                            class="flex h-full w-full flex-col items-center justify-center gap-2"
                                        >
                                            <span
                                                class="flex size-12 items-center justify-center rounded-pill bg-accent pl-0.5"
                                            >
                                                <Icon
                                                    name="play"
                                                    size={20}
                                                    color="var(--color-base)"
                                                />
                                            </span>
                                            <span
                                                class="text-xs tracking-[0.06em] text-ink-3 uppercase"
                                                >Thumbnail unavailable</span
                                            >
                                        </div>
                                    {/if}
                                </div>
                                <div class="flex flex-col gap-1">
                                    <h2
                                        class="m-0 line-clamp-2 text-sm leading-5 font-bold text-ink"
                                    >
                                        {previewTitle}
                                    </h2>
                                    <span class="text-xs tracking-[0.06em] text-ink-3 uppercase"
                                        >{previewChannel}{previewChannel === 'YouTube'
                                            ? ''
                                            : ' · YouTube'}</span
                                    >
                                    {#if app.videoPreparing(videoId ?? '')}
                                        <span
                                            class="text-xs tracking-[0.06em] text-accent uppercase"
                                            >Preparing video…</span
                                        >
                                    {/if}
                                    {#if preview.status === 'loading'}
                                        <div
                                            class="mt-1 flex flex-col gap-1.5"
                                            aria-label="Loading video details"
                                        >
                                            <span
                                                class="h-3 w-full animate-pulse rounded-sm bg-line motion-reduce:animate-none"
                                            ></span>
                                            <span
                                                class="h-3 w-3/4 animate-pulse rounded-sm bg-line motion-reduce:animate-none"
                                            ></span>
                                        </div>
                                    {:else if preview.status === 'error'}
                                        <p class="m-0 text-step-xs leading-[1.5] text-ink-3">
                                            Preview unavailable. You can still send the link.
                                        </p>
                                    {:else if previewDescription !== ''}
                                        <p
                                            class="m-0 line-clamp-3 text-step-xs leading-[1.5] text-ink-3"
                                        >
                                            {previewDescription}
                                        </p>
                                    {/if}
                                </div>
                            </div>
                        {:else}
                            <span
                                class="flex size-[88px] items-center justify-center rounded-pill bg-accent pl-1"
                                ><Icon name="play" size={32} color="var(--color-base)" /></span
                            >
                        {/if}
                        <label class="sr-only" for="watch-video">YouTube link</label>
                        <input
                            id="watch-video"
                            class="h-10 w-full max-w-[520px] rounded-sm border border-line bg-surface px-3 text-step-xs text-ink outline-none placeholder:text-ink-3 focus:border-line-strong"
                            bind:value={videoInput}
                            placeholder="https://youtube.com/watch?v=..."
                            aria-invalid={videoInput !== '' && videoId === null}
                        />
                        {#if videoInput !== '' && videoId === null}
                            <p class="m-0 text-step-xs text-error">
                                That link does not contain a YouTube video.
                            </p>
                        {:else}
                            <p class="m-0 text-step-xs text-ink-3">
                                Playback stays inside the watch party page.
                            </p>
                        {/if}
                    </div>
                    <div class="flex flex-wrap gap-2 px-1 pt-3">
                        <button
                            class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-accent bg-accent px-[18px] text-step-xs leading-none font-bold text-base transition-[background-color,border-color,color] duration-150 ease-soft hover:border-ink-2 hover:bg-ink-2 disabled:cursor-not-allowed disabled:opacity-50"
                            onclick={start}
                            disabled={selected.length === 0 || videoId === null}
                        >
                            Start watch party
                        </button>
                        <button
                            class="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-pill border border-line bg-transparent px-[18px] text-step-xs leading-none font-bold text-ink transition-[background-color,border-color,color] duration-150 ease-soft hover:border-line-strong hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-50"
                            onclick={sendVideo}
                            disabled={selected.length === 0 || videoId === null}
                        >
                            Send video link
                        </button>
                    </div>
                </div>

                <aside class="flex w-80 shrink-0 flex-col gap-4">
                    <section
                        class="flex min-h-0 flex-1 flex-col rounded-md border border-line bg-surface px-4 py-3.5"
                    >
                        <div class="mb-1 flex items-center justify-between gap-2">
                            <h2
                                class="m-0 font-text text-xs leading-4 font-bold tracking-[0.08em] text-ink-3 uppercase"
                            >
                                People · {selected.length}
                            </h2>
                            <span class="text-xs leading-4 text-ink-3 tabular-nums"
                                >{onlineCount} online</span
                            >
                        </div>
                        <label
                            class="flex h-9 items-center gap-2 rounded-sm border border-line bg-base px-3 text-step-xs text-ink-3 focus-within:border-line-strong"
                        >
                            <Icon name="search" size={15} color="var(--color-ink-3)" />
                            <input
                                class="min-w-0 flex-1 border-0 bg-transparent text-step-xs text-ink outline-none placeholder:text-ink-3"
                                bind:this={peopleSearch}
                                bind:value={query}
                                placeholder="Search people"
                                aria-label="Search people"
                            />
                            <span class="font-text text-xs tracking-[0.06em] text-ink-3 uppercase"
                                >/</span
                            >
                        </label>
                        <div class="mt-1 flex min-h-0 flex-1 flex-col overflow-y-auto">
                            {#each visibleUsers as user (user.id)}
                                {@const isSelected = selected.includes(user.id)}
                                <label
                                    class="flex cursor-pointer items-center gap-2.5 border-t border-line py-2.5 {user.connected
                                        ? ''
                                        : 'cursor-not-allowed opacity-40'}"
                                >
                                    <input
                                        class="peer sr-only"
                                        type="checkbox"
                                        checked={isSelected}
                                        disabled={!user.connected}
                                        onchange={() => toggleSelected(user.id)}
                                        aria-label={`Select ${shortName(user)}`}
                                    />
                                    <span
                                        class="flex size-[18px] shrink-0 items-center justify-center rounded-pill border border-line-strong text-transparent transition-[background-color,border-color,color,box-shadow] duration-150 ease-soft peer-checked:border-accent peer-checked:bg-accent peer-checked:text-base peer-focus-visible:ring-2 peer-focus-visible:ring-accent peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-surface"
                                    >
                                        <Icon name="check" size={10} color="currentColor" />
                                    </span>
                                    <Avatar
                                        {user}
                                        image={app.avatarUrl(user.fingerprint)}
                                        size={32}
                                    />
                                    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                                        <strong class="truncate text-sm leading-[18px]"
                                            >{shortName(user)}</strong
                                        >
                                        <span
                                            class="flex items-center gap-1.5 text-xs tracking-[0.06em] text-ink-3 uppercase"
                                            ><span
                                                class="size-1.5 rounded-full"
                                                class:bg-success={user.connected}
                                                class:bg-line-strong={!user.connected}
                                            ></span>{user.connected ? 'Online' : 'Offline'}</span
                                        >
                                    </span>
                                </label>
                            {/each}
                            {#if visibleUsers.length === 0}
                                <p class="m-0 py-3 text-step-xs text-ink-3">
                                    {onlineUsers.length === 0
                                        ? 'No users discovered yet.'
                                        : `No one matches “${query.trim()}”.`}
                                </p>
                            {/if}
                        </div>
                        <p
                            class="mt-3 mb-0 shrink-0 border-t border-line pt-3 text-xs leading-[1.5] tracking-[0.06em] text-ink-3 uppercase"
                        >
                            Start a group party for everyone selected, or send the video link as a
                            message.
                        </p>
                    </section>
                </aside>
            {:else}
                <div
                    class="flex min-w-0 flex-1 flex-col rounded-md border border-line bg-surface p-3"
                >
                    <div class="flex flex-col gap-0.5 px-1 pb-2.5">
                        <h1 class="m-0 leading-5 font-bold text-base text-ink">{playerTitle}</h1>
                        <span
                            class="font-text text-xs leading-4 tracking-[0.06em] text-ink-3 uppercase"
                            >YouTube · {party.participants.length} watching</span
                        >
                    </div>

                    {#if guestInvite}
                        <div
                            class="flex min-h-[300px] flex-1 flex-col items-center justify-center gap-2.5 rounded-sm bg-surface-2 p-8 text-center"
                        >
                            <span
                                class="flex size-[88px] items-center justify-center rounded-pill bg-accent pl-1"
                                ><Icon name="play" size={32} color="var(--color-base)" /></span
                            >
                            <h2 class="mt-2 mb-0 font-display text-step-2 font-normal">
                                {app.userName(party.host_id)} invited you
                            </h2>
                            <p class="m-0 text-step-xs text-ink-3">
                                Join the watch party for {playerTitle}.
                            </p>
                            {#if app.videoPreparing(party.video_id)}
                                <p class="m-0 text-step-xs text-accent">Preparing video…</p>
                            {/if}
                            <div class="mt-2 flex gap-2.5">
                                <button
                                    class="inline-flex min-h-9 items-center justify-center rounded-pill border border-error bg-transparent px-[18px] text-step-xs font-bold text-error hover:bg-error/12"
                                    onclick={() => void app.declineParty()}>Decline</button
                                >
                                <button
                                    class="inline-flex min-h-9 items-center justify-center rounded-pill border border-accent bg-accent px-[18px] text-step-xs font-bold text-base hover:border-ink-2 hover:bg-ink-2"
                                    onclick={() => void app.acceptParty()}>Join</button
                                >
                            </div>
                        </div>
                    {:else}
                        {#if waiting}
                            <div
                                class="mx-1 mb-2 rounded-sm border border-line bg-surface-2 px-3 py-2 text-step-xs text-ink-2"
                            >
                                Waiting for {invited.length} invited
                                {invited.length === 1 ? 'person' : 'people'} to accept.
                            </div>
                        {/if}
                        <YoutubePlayer
                            {app}
                            {party}
                            {selfId}
                            onPublish={(positionMs, playing) =>
                                void app.publishParty(positionMs, playing)}
                        />
                    {/if}
                </div>

                <aside class="flex w-80 shrink-0 flex-col gap-4">
                    <section class="rounded-md border border-line bg-surface px-4 py-3.5">
                        <h2
                            class="m-0 mb-1 font-text text-xs leading-4 font-bold tracking-[0.08em] text-ink-3 uppercase"
                        >
                            Watching · {participants.length}
                        </h2>
                        {#each participants as person (person.id)}
                            <div class="flex items-center gap-2.5 border-t border-line py-2.5">
                                <Avatar name={person.name} image={person.avatar} size={32} />
                                <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                                    <strong class="truncate text-sm leading-[18px]"
                                        >{person.name}</strong
                                    >
                                    <span
                                        class="flex items-center gap-1.5 text-xs tracking-[0.06em] text-ink-3 uppercase"
                                        ><span
                                            class="size-1.5 rounded-full"
                                            class:bg-success={person.connected}
                                            class:bg-line-strong={!person.connected}
                                        ></span>{person.host
                                            ? 'Started the party'
                                            : person.self
                                              ? 'You'
                                              : 'Watching'}</span
                                    >
                                </div>
                                {#if controlling === person.id && party.state === 'active'}
                                    <span
                                        class="rounded-pill bg-accent px-2.5 py-1 text-xs font-bold text-base"
                                        >Controlling</span
                                    >
                                {/if}
                            </div>
                        {/each}
                        <p
                            class="mt-1 mb-0 border-t border-line pt-3 text-xs leading-[1.5] tracking-[0.06em] text-ink-3 uppercase"
                        >
                            Anyone can play, pause, or seek. Changes apply to all.
                        </p>
                    </section>

                    {#if invited.length > 0}
                        <section class="rounded-md border border-line bg-surface px-4 py-3.5">
                            <h2
                                class="m-0 mb-1 font-text text-xs leading-4 font-bold tracking-[0.08em] text-ink-3 uppercase"
                            >
                                Invited · {invited.length}
                            </h2>
                            {#each invited as person (person.id)}
                                <div class="flex items-center gap-2.5 border-t border-line py-2.5">
                                    <Avatar name={person.name} image={person.avatar} size={28} />
                                    <span class="min-w-0 flex-1 truncate text-sm"
                                        >{person.name}</span
                                    >
                                    <span class="text-xs tracking-[0.06em] text-ink-3 uppercase"
                                        >Pending</span
                                    >
                                </div>
                            {/each}
                        </section>
                    {/if}

                    {#if isHost}
                        <section class="rounded-md border border-line bg-surface px-4 py-3.5">
                            <button
                                class="flex w-full items-center justify-between text-left"
                                onclick={() => (inviteOpen = !inviteOpen)}
                                aria-expanded={inviteOpen}
                            >
                                <span
                                    class="font-text text-xs leading-4 font-bold tracking-[0.08em] text-ink-3 uppercase"
                                    >Invite people</span
                                >
                                <Icon
                                    name={inviteOpen ? 'chevron-left' : 'chevron-right'}
                                    size={14}
                                />
                            </button>
                            {#if inviteOpen}
                                <div class="mt-3 flex flex-col gap-2">
                                    <label
                                        class="flex h-9 items-center gap-2 rounded-sm border border-line bg-base px-3 text-step-xs text-ink-3 focus-within:border-line-strong"
                                    >
                                        <Icon name="search" size={15} color="var(--color-ink-3)" />
                                        <input
                                            class="min-w-0 flex-1 border-0 bg-transparent text-step-xs text-ink outline-none placeholder:text-ink-3"
                                            bind:this={inviteSearch}
                                            bind:value={inviteQuery}
                                            placeholder="Search invitees"
                                            aria-label="Search invitees"
                                        />
                                        <span
                                            class="font-text text-xs tracking-[0.06em] text-ink-3 uppercase"
                                            >/</span
                                        >
                                    </label>
                                    <div class="flex max-h-80 flex-col overflow-y-auto">
                                        {#each visibleInvitable as user (user.id)}
                                            <label
                                                class="flex cursor-pointer items-center gap-2.5 py-1.5"
                                            >
                                                <input
                                                    class="peer sr-only"
                                                    type="checkbox"
                                                    checked={inviteSelected.includes(user.id)}
                                                    onchange={() => toggleInvite(user.id)}
                                                    aria-label={`Invite ${shortName(user)}`}
                                                />
                                                <span
                                                    class="flex size-[18px] shrink-0 items-center justify-center rounded-pill border border-line-strong text-transparent transition-[background-color,border-color,color,box-shadow] duration-150 ease-soft peer-checked:border-accent peer-checked:bg-accent peer-checked:text-base peer-focus-visible:ring-2 peer-focus-visible:ring-accent peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-surface"
                                                >
                                                    <Icon
                                                        name="check"
                                                        size={10}
                                                        color="currentColor"
                                                    />
                                                </span>
                                                <Avatar
                                                    {user}
                                                    image={app.avatarUrl(user.fingerprint)}
                                                    size={28}
                                                />
                                                <span class="min-w-0 flex-1 truncate text-sm"
                                                    >{shortName(user)}</span
                                                >
                                            </label>
                                        {/each}
                                        {#if visibleInvitable.length === 0}
                                            <p class="m-0 py-2 text-step-xs text-ink-3">
                                                {invitable.length === 0
                                                    ? 'Everyone online is already here or invited.'
                                                    : `No one matches “${inviteQuery.trim()}”.`}
                                            </p>
                                        {/if}
                                    </div>
                                    {#if invitable.length > 0}
                                        <button
                                            class="mt-2 inline-flex min-h-8 items-center justify-center rounded-pill border border-accent bg-accent px-3.5 text-step-xs font-bold text-base disabled:opacity-50"
                                            onclick={invite}
                                            disabled={inviteSelected.length === 0}
                                            >Invite selected</button
                                        >
                                    {/if}
                                </div>
                            {/if}
                        </section>
                    {/if}

                    <button
                        class="mt-1 inline-flex h-9 w-full items-center justify-center rounded-pill border border-error bg-transparent px-4 text-step-xs font-bold text-error transition-[background-color,border-color,color] duration-150 ease-soft hover:bg-error/12"
                        onclick={leave}
                    >
                        {party.role === 'host' ? 'End party' : 'Leave party'}
                    </button>

                    <div
                        class="mt-auto flex items-center gap-2 px-4 text-xs tracking-[0.06em] text-ink uppercase"
                    >
                        <span class="size-1.5 rounded-full bg-success"></span>
                        Synced · LAN
                    </div>
                </aside>
            {/if}
        </div>
    </div>
</section>

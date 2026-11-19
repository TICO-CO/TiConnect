<script lang="ts">
    interface Props {
        name:
            | 'search'
            | 'collapse'
            | 'expand'
            | 'settings'
            | 'call'
            | 'remote'
            | 'party'
            | 'back'
            | 'file'
            | 'play'
            | 'pause'
            | 'mute'
            | 'speaker'
            | 'paperclip'
            | 'mic'
            | 'send'
            | 'close'
            | 'check'
            | 'checks'
            | 'pointer'
            | 'keyboard'
            | 'fullscreen'
            | 'chevron-right'
            | 'chevron-left'
            | 'alert';
        size?: number;
        color?: string;
    }

    let { name, size = 16, color = 'var(--color-ink)' }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    {#if name === 'search'}
        <circle cx="11" cy="11" r="6.5" stroke={color} stroke-width="1.7" />
        <path d="m16 16 4 4" stroke={color} stroke-width="1.7" stroke-linecap="round" />
    {:else if name === 'collapse'}
        <path d="m14 6-6 6 6 6" stroke={color} stroke-width="1.7" stroke-linecap="round" />
    {:else if name === 'expand'}
        <path d="m10 6 6 6-6 6" stroke={color} stroke-width="1.7" stroke-linecap="round" />
    {:else if name === 'chevron-left'}
        <path d="m14 6-6 6 6 6" stroke={color} stroke-width="1.7" stroke-linecap="round" />
    {:else if name === 'chevron-right'}
        <path d="m10 6 6 6-6 6" stroke={color} stroke-width="1.7" stroke-linecap="round" />
    {:else if name === 'settings'}
        <path
            d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
            stroke={color}
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
        <circle cx="12" cy="12" r="3" stroke={color} stroke-width="1.5" />
    {:else if name === 'call'}
        <path
            d="M8.4 4.5 6.7 4a2 2 0 0 0-2.4 1.4l-.5 1.7c-.4 1.4.1 2.9 1.2 4a22 22 0 0 0 7.9 5.7c1.2.5 2.6.2 3.4-.8l1-1.1a2 2 0 0 0 .2-2.4l-1.2-1.7-2.2.8-2.4-2.4.8-2.2-1.7-1.2-2.4-.3Z"
            stroke={color}
            stroke-width="1.5"
            stroke-linejoin="round"
        />
    {:else if name === 'remote'}
        <rect x="3.5" y="5" width="17" height="11" rx="2" stroke={color} stroke-width="1.5" />
        <path d="M9 19.5h6M12 16v3.5" stroke={color} stroke-width="1.5" stroke-linecap="round" />
    {:else if name === 'party'}
        <circle cx="12" cy="12" r="8.5" stroke={color} stroke-width="1.5" />
        <path d="m10 8.5 5 3.5-5 3.5v-7Z" fill={color} />
    {:else if name === 'back'}
        <path
            d="M19 12H5M10 7l-5 5 5 5"
            stroke={color}
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
    {:else if name === 'file'}
        <path
            d="M6 3.5h7l5 5v12H6v-17Z"
            stroke={color}
            stroke-width="1.5"
            stroke-linejoin="round"
        />
        <path d="M13 3.5v5h5" stroke={color} stroke-width="1.5" stroke-linejoin="round" />
    {:else if name === 'play'}
        <path d="m9 7 8 5-8 5V7Z" fill={color} />
    {:else if name === 'pause'}
        <path d="M9 7v10M15 7v10" stroke={color} stroke-width="2" stroke-linecap="round" />
    {:else if name === 'mute'}
        <path
            d="M5 10v4h3l4 3V7l-4 3H5Z"
            stroke={color}
            stroke-width="1.5"
            stroke-linejoin="round"
        />
        <path d="m16 10 4 4M20 10l-4 4" stroke={color} stroke-width="1.5" stroke-linecap="round" />
    {:else if name === 'speaker'}
        <path
            d="M5 10v4h3l4 3V7l-4 3H5Z"
            stroke={color}
            stroke-width="1.5"
            stroke-linejoin="round"
        />
        <path d="M16 9a4 4 0 0 1 0 6" stroke={color} stroke-width="1.5" stroke-linecap="round" />
    {:else if name === 'paperclip'}
        <path
            d="m9 12 5.2-5.2a3 3 0 0 1 4.3 4.3l-6.8 6.8a4.5 4.5 0 0 1-6.4-6.4l6-6"
            stroke={color}
            stroke-width="1.5"
            stroke-linecap="round"
        />
    {:else if name === 'mic'}
        <rect x="9.5" y="4" width="5" height="9" rx="2.5" stroke={color} stroke-width="1.5" />
        <path
            d="M7 11a5 5 0 0 0 10 0M12 16v4M9.5 20h5"
            stroke={color}
            stroke-width="1.5"
            stroke-linecap="round"
        />
    {:else if name === 'send'}
        <path
            d="M5 12h13M13 6l6 6-6 6"
            stroke={color}
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
    {:else if name === 'close'}
        <path d="m6 6 12 12M18 6 6 18" stroke={color} stroke-width="1.6" stroke-linecap="round" />
    {:else if name === 'check'}
        <path
            d="m5 12 4 4 10-10"
            stroke={color}
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
    {:else if name === 'checks'}
        <path
            d="m3 12 3.5 3.5L13 9M11 15.5 12.5 17 21 8.5"
            stroke={color}
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
    {:else if name === 'pointer'}
        <path
            d="m7 4 10 8-4.5 1.2L10 19 7 4Z"
            stroke={color}
            stroke-width="1.5"
            stroke-linejoin="round"
        />
    {:else if name === 'keyboard'}
        <rect x="3" y="7" width="18" height="10" rx="2" stroke={color} stroke-width="1.5" />
        <path
            d="M7 11h.01M10 11h.01M13 11h.01M16 11h.01M8 14h8"
            stroke={color}
            stroke-width="1.8"
            stroke-linecap="round"
        />
    {:else if name === 'fullscreen'}
        <path
            d="M9 4H4v5M15 4h5v5M9 20H4v-5M20 15v5h-5"
            stroke={color}
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
        />
    {:else if name === 'alert'}
        <circle cx="12" cy="12" r="8.5" stroke={color} stroke-width="1.5" />
        <path d="M12 8v4.5M12 16h.01" stroke={color} stroke-width="1.8" stroke-linecap="round" />
    {/if}
</svg>

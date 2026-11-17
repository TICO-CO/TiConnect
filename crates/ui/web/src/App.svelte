<script lang="ts">
    import { onMount } from 'svelte';

    import type { Bootstrap, WebSocketEvent } from './lib';
    import { BridgeClient, resolveBridge } from './lib';

    type Connection = 'connecting' | 'online' | 'offline';

    let connection = $state<Connection>('connecting');
    let bootstrap = $state<Bootstrap | null>(null);
    let errorMessage = $state('');

    onMount(() => {
        const client = new BridgeClient(resolveBridge(new URL(window.location.href)));
        const stopEvents = client.onEvent((event: WebSocketEvent) => {
            switch (event.type) {
                case 'connected':
                    connection = 'online';
                    break;
                case 'disconnected':
                    connection = 'offline';
                    break;
                default:
                    break;
            }
        });

        void client.connect().then(async ([, connectFailure]) => {
            if (connectFailure) {
                connection = 'offline';
                errorMessage = connectFailure.message;
                return;
            }
            const [bootstrapResult, bootstrapFailure] =
                await client.request<Bootstrap>('bootstrap');
            if (bootstrapFailure) {
                connection = 'offline';
                errorMessage = bootstrapFailure.message;
                return;
            }
            if (bootstrapResult === null) {
                connection = 'offline';
                errorMessage = 'bootstrap response was empty';
                return;
            }
            bootstrap = bootstrapResult;
            connection = 'online';
        });

        return () => {
            stopEvents();
            client.close();
        };
    });

    const title = $derived(
        bootstrap?.identity.display_name?.trim() || bootstrap?.identity.node_id || 'TiConnect',
    );
</script>

<main>
    <header>
        <div>
            <p class="eyebrow">TiConnect</p>
            <h1>{title}</h1>
        </div>
        <span class="status" data-state={connection}>{connection}</span>
    </header>

    {#if errorMessage}
        <section class="panel error">
            <h2>Bridge unavailable</h2>
            <p>{errorMessage}</p>
        </section>
    {:else if bootstrap}
        <section class="panel">
            <dl>
                <div>
                    <dt>Version</dt>
                    <dd>{bootstrap.version}</dd>
                </div>
                <div>
                    <dt>Transport</dt>
                    <dd>{bootstrap.status.transport_state || 'unknown'}</dd>
                </div>
                <div>
                    <dt>Peers</dt>
                    <dd>{bootstrap.status.peer_count}</dd>
                </div>
                <div>
                    <dt>Fingerprint</dt>
                    <dd class="mono">{bootstrap.identity.fingerprint}</dd>
                </div>
            </dl>
        </section>
    {:else}
        <section class="panel">
            <p>Connecting to the TiConnect daemon…</p>
        </section>
    {/if}

    <footer>
        <p>Svelte stub. Replace this screen with the real UI.</p>
    </footer>
</main>

#!/usr/bin/env bun
import { readFileSync } from 'node:fs';

const REQUEST_TIMEOUT_MS = 15_000;

function deferred() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

class Bridge {
    constructor(name, socket) {
        this.name = name;
        this.socket = socket;
        this.nextId = 1;
        this.pending = new Map();
        this.events = [];
        this.waiters = [];
        this.opened = deferred();
        this.ready = deferred();
        this.closed = deferred();
        socket.onopen = () => this.opened.resolve();
        socket.onerror = () => this.fail(new Error(`${name} bridge socket error`));
        socket.onclose = () => {
            this.closed.resolve();
            this.fail(new Error(`${name} bridge socket closed`));
        };
        socket.onmessage = (event) => this.handle(event.data);
    }

    static async connect(name, path) {
        const raw = readFileSync(path, 'utf8').trim();
        const url = new URL(raw);
        const token = new URLSearchParams(url.hash.replace(/^#/, '')).get('token');
        const bridgeHost = url.searchParams.get('bridge') ?? url.host;
        const socket = new WebSocket(`ws://${bridgeHost}/ws`, {
            headers: { Origin: `http://${bridgeHost}` },
        });
        const bridge = new Bridge(name, socket);
        await bridge.opened.promise;
        socket.send(JSON.stringify({ type: 'auth', ...(token ? { token } : {}) }));
        await bridge.ready.promise;
        return bridge;
    }

    fail(error) {
        this.ready.reject(error);
        for (const pending of this.pending.values()) {
            clearTimeout(pending.timer);
            pending.reject(error);
        }
        this.pending.clear();
        for (const waiter of this.waiters) {
            clearTimeout(waiter.timer);
            waiter.reject(error);
        }
        this.waiters = [];
    }

    handle(raw) {
        let message;
        try {
            message = JSON.parse(raw);
        } catch (error) {
            this.fail(new Error(`${this.name} bridge sent invalid JSON: ${error.message}`));
            return;
        }
        if (message.type === 'ready') {
            this.ready.resolve();
            return;
        }
        if (message.type === 'error' && message.id == null) {
            this.fail(new Error(`${this.name} bridge rejected auth: ${message.message ?? message.code}`));
            return;
        }
        if (message.id != null) {
            const pending = this.pending.get(Number(message.id));
            if (pending === undefined) {
                return;
            }
            this.pending.delete(Number(message.id));
            clearTimeout(pending.timer);
            if (message.ok) {
                pending.resolve(message.result);
            } else {
                pending.reject(
                    new Error(
                        `${this.name} ${pending.method} failed: ${message.error?.message ?? 'unknown error'}`,
                    ),
                );
            }
            return;
        }
        this.events.push(message);
        this.flush();
    }

    flush() {
        for (let index = 0; index < this.events.length; index += 1) {
            const event = this.events[index];
            const waiterIndex = this.waiters.findIndex((waiter) => waiter.predicate(event));
            if (waiterIndex === -1) {
                continue;
            }
            const [waiter] = this.waiters.splice(waiterIndex, 1);
            clearTimeout(waiter.timer);
            this.events.splice(index, 1);
            index -= 1;
            waiter.resolve(event);
        }
    }

    request(method, params = {}) {
        const id = this.nextId;
        this.nextId += 1;
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                this.pending.delete(id);
                reject(new Error(`${this.name} ${method} timed out`));
            }, REQUEST_TIMEOUT_MS);
            this.pending.set(id, { method, resolve, reject, timer });
            try {
                this.socket.send(JSON.stringify({ id, method, params }));
            } catch (error) {
                this.pending.delete(id);
                clearTimeout(timer);
                reject(error);
            }
        });
    }

    waitEvent(predicate, label, timeoutMs = REQUEST_TIMEOUT_MS) {
        const existing = this.events.findIndex(predicate);
        if (existing !== -1) {
            return Promise.resolve(this.events.splice(existing, 1)[0]);
        }
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                this.waiters = this.waiters.filter((waiter) => waiter.timer !== timer);
                reject(new Error(`${this.name} waited for ${label}`));
            }, timeoutMs);
            this.waiters.push({ predicate, resolve, reject, timer });
        });
    }

    close() {
        this.socket.close();
    }
}

async function waitFor(label, predicate, timeoutMs = REQUEST_TIMEOUT_MS) {
    const deadline = Date.now() + timeoutMs;
    let lastError = null;
    while (Date.now() < deadline) {
        try {
            if (await predicate()) {
                return;
            }
        } catch (error) {
            lastError = error;
        }
        await sleep(100);
    }
    throw new Error(`${label} timed out${lastError ? `: ${lastError.message}` : ''}`);
}

function activeCall(bridge) {
    return bridge.request('call_state');
}

async function waitForState(bridge, expected, timeoutMs = REQUEST_TIMEOUT_MS) {
    await waitFor(
        `${bridge.name} state ${expected ?? 'idle'}`,
        async () => {
            const call = await activeCall(bridge);
            return expected === null ? call === null : call?.state === expected;
        },
        timeoutMs,
    );
}

async function assertStillInCall(bridge, label) {
    const call = await activeCall(bridge);
    if (call?.state !== 'in-call') {
        throw new Error(`${label} left in-call: ${JSON.stringify(call)}`);
    }
}

async function waitForAudio(bridge, timeoutMs = REQUEST_TIMEOUT_MS) {
    await waitFor(
        `${bridge.name} audio metrics`,
        async () => {
            const metrics = await bridge.request('call_metrics');
            return Number(metrics.frames_sent) > 0 && Number(metrics.frames_received) > 0;
        },
        timeoutMs,
    );
}

async function runDirection(caller, callee, callerUserId, calleeUserId, label) {
    const incoming = callee.waitEvent(
        (event) => event.type === 'incoming_call' && event.call?.user_id === callerUserId,
        'incoming call',
    );
    const callId = await caller.request('start_call', { user_id: calleeUserId });
    if (typeof callId !== 'string' || callId.length === 0) {
        throw new Error(`${label} start_call returned invalid id: ${JSON.stringify(callId)}`);
    }
    const incomingCall = await incoming;
    if (incomingCall.call?.id !== callId) {
        throw new Error(
            `${label} incoming call id mismatch: ${incomingCall.call?.id} != ${callId}`,
        );
    }

    await callee.request('accept_call', { call_id: callId });
    await Promise.all([waitForState(caller, 'in-call'), waitForState(callee, 'in-call')]);
    await sleep(1_500);
    await Promise.all([
        assertStillInCall(caller, `${label} caller`),
        assertStillInCall(callee, `${label} callee`),
    ]);
    await Promise.all([waitForAudio(caller), waitForAudio(callee)]);

    const callerMetrics = await caller.request('call_metrics');
    const calleeMetrics = await callee.request('call_metrics');
    console.log(
        `${label} audio caller sent=${callerMetrics.frames_sent} received=${callerMetrics.frames_received} callee sent=${calleeMetrics.frames_sent} received=${calleeMetrics.frames_received}`,
    );

    await caller.request('hangup_call', { call_id: callId });
    await Promise.all([waitForState(caller, null), waitForState(callee, null)]);
    console.log(`${label} hung up cleanly`);
}

async function main() {
    const aFile = process.env.BRIDGE_A_FILE;
    const bFile = process.env.BRIDGE_B_FILE;
    const aUserId = process.env.NODE_A_ID;
    const bUserId = process.env.NODE_B_ID;
    if (!aFile || !bFile || !aUserId || !bUserId) {
        throw new Error('BRIDGE_A_FILE, BRIDGE_B_FILE, NODE_A_ID and NODE_B_ID are required');
    }
    const a = await Bridge.connect('node-a', aFile);
    const b = await Bridge.connect('node-b', bFile);
    try {
        await runDirection(a, b, aUserId, bUserId, 'a->b');
        await runDirection(b, a, bUserId, aUserId, 'b->a');
    } finally {
        a.close();
        b.close();
    }
    console.log('main-window call e2e passed');
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error instanceof Error ? error.stack ?? error.message : String(error));
        process.exit(1);
    });

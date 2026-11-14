# Performance

Targets are for a modern laptop on a wired or 5 GHz LAN.

| Metric | Target |
|---|---|
| Idle daemon RSS | under 30 MB |
| Overlay idle RSS | about 40 MB |
| Idle CPU | under 1% |
| Remote desktop | 1080p software H.264 |
| 4K remote desktop | out of scope |

## Measuring

```sh
scripts/bench/idle.sh
scripts/bench/startup.sh
scripts/bench/messaging.sh <peer-id> 20
scripts/bench/transfer.sh <peer-id> 64
scripts/bench/call.sh <peer-id> 15
scripts/bench/remote.sh <peer-id> 15
```

Record the hardware, kernel, Rust toolchain, desktop, and session type with every
measurement. Compare runs on the same machine.

Idle verification uses `strace -c -f -p <pid>` or `perf top -p <pid>` for a
minute; there must be no busy loop and no periodic wakeup faster than the
configured discovery interval.

Attachment chunks default to 64 KiB with a 2 GiB maximum. Calls use 20 ms Opus
frames. Video targets 1080p at the configured bitrate and frame rate, dropping
frames under backpressure.

## Tuning

| Key | Default | Meaning |
|---|---|---|
| `attachments.max_size_mib` | 2048 | maximum accepted attachment |
| `calls.timeout_secs` | 30 | ringing timeout |
| `video.bitrate_kbps` | 2500 | H.264 target bitrate |
| `video.fps` | 30 | capture and encode frame rate |
| `video.keyframe_interval` | 60 | frames between keyframes |
| `remote.session_timeout_secs` | 0 | 0 disables automatic timeout |
| `discovery.beacon_interval_secs` | 5 | idle LAN beacon interval |

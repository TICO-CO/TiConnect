use std::f64::consts::{SQRT_2, TAU};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ticonnect_core::call::{AudioPacket, CallId};
use ticonnect_media::audio::{
    AudioPlayout, OPUS_FRAME_SAMPLES, OPUS_SAMPLE_RATE, Resampler, start_capture,
};
use ticonnect_media::opus_codec::{OpusDecoder, OpusEncoder};

const RESAMPLE_TOLERANCE: f32 = 1e-6;
const ALIAS_FLOOR_DBFS: f64 = -60.0;

fn tone(rate: u32, frequency: f64, seconds: f64, amplitude: f32) -> Vec<f32> {
    let samples = (f64::from(rate) * seconds).round() as usize;
    (0..samples)
        .map(|index| {
            let phase = TAU * frequency * index as f64 / f64::from(rate);
            (phase.sin() as f32) * amplitude
        })
        .collect()
}

fn process_chunked(resampler: &mut Resampler, input: &[f32]) -> Vec<f32> {
    const SIZES: [usize; 6] = [97, 311, 480, 1024, 64, 777];
    let mut output = Vec::new();
    let mut offset = 0;
    let mut index = 0;
    while offset < input.len() {
        let size = SIZES[index % SIZES.len()].min(input.len() - offset);
        output.extend(
            resampler
                .process(&input[offset..offset + size])
                .expect("chunked resample"),
        );
        offset += size;
        index += 1;
    }
    output
}

fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>();
    (energy / samples.len() as f64).sqrt()
}

fn dbfs(value: f64) -> f64 {
    20.0 * value.max(1e-12).log10()
}

fn tone_amplitude(samples: &[f32], rate: u32, frequency: f64) -> f64 {
    let mut real = 0.0;
    let mut imaginary = 0.0;
    let step = TAU * frequency / f64::from(rate);
    for (index, sample) in samples.iter().enumerate() {
        let phase = step * index as f64;
        real += f64::from(*sample) * phase.cos();
        imaginary -= f64::from(*sample) * phase.sin();
    }
    let magnitude = (real * real + imaginary * imaginary).sqrt() * 2.0 / samples.len() as f64;
    magnitude / SQRT_2
}

fn best_snr(reference: &[f32], actual: &[f32], max_lag: usize) -> f64 {
    let mut best = f64::NEG_INFINITY;
    for lag in 0..=max_lag {
        let available = actual.len().saturating_sub(lag);
        let length = reference.len().min(available);
        if length < 4_800 {
            continue;
        }
        let mut signal = 0.0;
        let mut noise = 0.0;
        for index in 0..length {
            let expected = f64::from(reference[index]);
            let observed = f64::from(actual[lag + index]);
            signal += expected * expected;
            let error = observed - expected;
            noise += error * error;
        }
        let snr = 10.0 * (signal / noise.max(1e-20)).log10();
        if snr > best {
            best = snr;
        }
    }
    best
}

#[test]
fn resampler_chunked_matches_whole_input() {
    let pairs = [
        (8_000, 48_000),
        (16_000, 48_000),
        (32_000, 48_000),
        (44_100, 48_000),
        (96_000, 48_000),
        (48_000, 16_000),
        (48_000, 32_000),
        (48_000, 44_100),
    ];
    for (input_rate, output_rate) in pairs {
        let input = tone(input_rate, 440.0, 1.0, 0.25);
        let mut whole = Resampler::new(input_rate, output_rate).expect("whole resampler");
        let whole_output = whole.process(&input).expect("whole process");
        let mut chunked = Resampler::new(input_rate, output_rate).expect("chunked resampler");
        let chunked_output = process_chunked(&mut chunked, &input);
        assert_eq!(whole_output.len(), chunked_output.len());
        for (index, (expected, observed)) in
            whole_output.iter().zip(chunked_output.iter()).enumerate()
        {
            let difference = (expected - observed).abs();
            assert!(
                difference <= RESAMPLE_TOLERANCE,
                "{input_rate}->{output_rate} sample {index}: {expected} != {observed}"
            );
        }
    }
}

#[test]
fn resampler_rejects_downsampling_aliases() {
    let input = tone(48_000, 10_000.0, 1.0, 0.25);
    let mut resampler = Resampler::new(48_000, 16_000).expect("resampler");
    let output = resampler.process(&input).expect("resample");
    let window = &output[1_000..13_800];
    let alias = tone_amplitude(window, 16_000, 6_000.0);
    let alias_dbfs = dbfs(alias);
    assert!(
        alias_dbfs < ALIAS_FLOOR_DBFS,
        "alias at 6 kHz is {alias_dbfs:.1} dBFS"
    );
}

#[test]
fn opus_round_trip_keeps_signal_and_silence_clean() {
    let input = tone(OPUS_SAMPLE_RATE, 440.0, 1.0, 0.3);
    let mut encoder = OpusEncoder::default_voip().expect("encoder");
    let packets = encoder.push_samples(&input).expect("encode");
    let mut decoder = OpusDecoder::default_voip().expect("decoder");
    let mut output = Vec::new();
    for packet in packets {
        output.extend(decoder.decode(&packet).expect("decode"));
    }
    let snr = best_snr(&input, &output, 2_048);
    assert!(snr >= 15.0, "Opus round-trip SNR is {snr:.1} dB");

    let mut silence_encoder = OpusEncoder::default_voip().expect("silence encoder");
    let mut silence_decoder = OpusDecoder::default_voip().expect("silence decoder");
    let silence = vec![0.0; OPUS_FRAME_SAMPLES * 10];
    let packets = silence_encoder
        .push_samples(&silence)
        .expect("encode silence");
    let mut silence_output = Vec::new();
    for packet in packets {
        silence_output.extend(silence_decoder.decode(&packet).expect("decode silence"));
    }
    let settled = &silence_output[OPUS_FRAME_SAMPLES * 5..];
    let noise_floor = dbfs(rms(settled));
    assert!(
        noise_floor < -60.0,
        "silence noise floor is {noise_floor:.1} dBFS"
    );
}

#[test]
fn wired_48k_chain_preserves_signal() {
    let input = tone(48_000, 440.0, 1.0, 0.3);
    let mut encoder = OpusEncoder::default_voip().expect("encoder");
    let payloads = encoder.push_samples(&input).expect("encode");
    let call_id = CallId::generate();
    let mut playout = AudioPlayout::new().expect("playout");
    let start = Instant::now();
    for (index, payload) in payloads.iter().enumerate() {
        playout.push_at(
            AudioPacket::new(
                call_id,
                u32::try_from(index).expect("sequence"),
                u32::try_from(index * OPUS_FRAME_SAMPLES).expect("timestamp"),
                payload.clone(),
            ),
            start + Duration::from_millis(20 * index as u64),
        );
    }
    let mut output = Vec::new();
    for _ in 0..70 {
        if let Some(samples) = playout.tick().expect("tick") {
            output.extend(samples);
        }
    }
    output.truncate(input.len());
    let snr = best_snr(&input[..output.len()], &output, 2_048);
    assert!(snr >= 15.0, "wired chain SNR is {snr:.1} dB");
}

#[test]
fn bluetooth_rate_chain_conceals_jitter_and_loss() {
    let input = tone(16_000, 440.0, 2.0, 0.25);
    let mut capture_resampler = Resampler::new(16_000, 48_000).expect("capture resampler");
    let mut pcm = Vec::new();
    for chunk in input.chunks(160) {
        pcm.extend(capture_resampler.process(chunk).expect("capture resample"));
    }
    let mut encoder = OpusEncoder::default_voip().expect("encoder");
    let payloads = encoder.push_samples(&pcm).expect("encode");
    let call_id = CallId::generate();
    let mut playout = AudioPlayout::new().expect("playout");
    let start = Instant::now();
    let mut sequence = 0_u32;
    for (index, payload) in payloads.into_iter().enumerate() {
        if index == 20 || index == 21 {
            sequence = sequence.wrapping_add(1);
            continue;
        }
        let arrival = start
            + Duration::from_millis(20 * index as u64)
            + Duration::from_millis((index % 4) as u64 * 15);
        let packet = AudioPacket::new(
            call_id,
            sequence,
            sequence.wrapping_mul(OPUS_FRAME_SAMPLES as u32),
            payload,
        );
        playout.push_at(packet, arrival);
        sequence = sequence.wrapping_add(1);
    }
    let mut decoded = Vec::new();
    for _ in 0..120 {
        if let Some(samples) = playout.tick().expect("tick") {
            decoded.extend(samples);
        }
    }
    assert!(playout.lost() >= 1, "loss was not detected");
    let mut playback_resampler = Resampler::new(48_000, 16_000).expect("playback resampler");
    let output = playback_resampler
        .process(&decoded)
        .expect("playback resample");
    assert!(output.len() > 16_000, "output too short: {}", output.len());
    assert!(output.iter().all(|sample| sample.is_finite()));
    let peak = output
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    assert!(peak <= 1.0, "output clipped at {peak}");
}

#[test]
#[ignore = "requires audio hardware; run with --ignored on each machine"]
fn hardware_microphone_probe() {
    let input_name = std::env::var("TICONNECT_AUDIO_TEST_INPUT").ok();
    let (sender, receiver) = mpsc::channel();
    let capture = start_capture(input_name.as_deref(), move |samples| {
        let _ = sender.send(samples);
    })
    .expect("capture device");
    println!(
        "device={} rate={} channels={} format={:?}",
        capture.name(),
        capture.sample_rate(),
        capture.channels(),
        capture.sample_format()
    );
    std::thread::sleep(Duration::from_secs(3));
    capture.stop();
    let mut samples = Vec::new();
    while let Ok(chunk) = receiver.try_recv() {
        samples.extend(chunk);
    }
    assert!(!samples.is_empty(), "no samples captured");
    let sum = samples.iter().map(|sample| f64::from(*sample)).sum::<f64>();
    let dc = sum / samples.len() as f64;
    let level = dbfs(rms(&samples));
    let peak = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let clipped = samples
        .iter()
        .filter(|sample| sample.abs() >= 0.999)
        .count() as f64
        / samples.len() as f64;
    println!(
        "rms={level:.1} dBFS peak={:.1} dBFS dc={dc:.5} clipped={:.3}%",
        dbfs(f64::from(peak)),
        clipped * 100.0
    );
    assert!(level > -60.0, "microphone is silent");
    assert!(dc.abs() < 0.05, "microphone has DC offset: {dc}");
    assert!(clipped < 0.01, "microphone is clipping: {:.2}%", clipped);
}

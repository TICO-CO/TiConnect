use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Resampler as RubatoResampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use thiserror::Error;

const SINC_LENGTH: usize = 256;
const OVERSAMPLING_FACTOR: usize = 128;
const MAX_RESAMPLE_RATIO_RELATIVE: f64 = 1.1;
const CHUNK_MS: u32 = 10;

#[derive(Debug)]
pub struct Resampler {
    input_rate: u32,
    output_rate: u32,
    inner: Option<Inner>,
    pending: Vec<f32>,
    input: Vec<f32>,
    output: Vec<f32>,
    delay_remaining: usize,
}

#[derive(Debug)]
struct Inner {
    resampler: Async<f32>,
    chunk_size: usize,
    output_capacity: usize,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, ResampleError> {
        if input_rate == 0 || output_rate == 0 {
            return Err(ResampleError::Config(format!(
                "sample rates must be non-zero: {input_rate} -> {output_rate}"
            )));
        }
        if input_rate == output_rate {
            return Ok(Self {
                input_rate,
                output_rate,
                inner: None,
                pending: Vec::new(),
                input: Vec::new(),
                output: Vec::new(),
                delay_remaining: 0,
            });
        }
        let chunk_size = usize::try_from((input_rate / 1000) * CHUNK_MS)
            .map_err(|error| ResampleError::Config(error.to_string()))?
            .max(64);
        let parameters = SincInterpolationParameters {
            sinc_len: SINC_LENGTH,
            f_cutoff: None,
            oversampling_factor: OVERSAMPLING_FACTOR,
            interpolation: SincInterpolationType::Cubic,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = f64::from(output_rate) / f64::from(input_rate);
        let resampler = Async::<f32>::new_sinc(
            ratio,
            MAX_RESAMPLE_RATIO_RELATIVE,
            &parameters,
            chunk_size,
            1,
            FixedAsync::Input,
        )
        .map_err(|error| ResampleError::Config(error.to_string()))?;
        let output_capacity = resampler.output_frames_max();
        let delay_remaining = resampler.output_delay();
        Ok(Self {
            input_rate,
            output_rate,
            inner: Some(Inner {
                resampler,
                chunk_size,
                output_capacity,
            }),
            pending: Vec::with_capacity(chunk_size * 2),
            input: Vec::with_capacity(chunk_size),
            output: vec![0.0; output_capacity],
            delay_remaining,
        })
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, ResampleError> {
        let Some(inner) = self.inner.as_mut() else {
            return Ok(input.to_vec());
        };
        self.pending.extend_from_slice(input);
        let ratio = f64::from(self.output_rate) / f64::from(self.input_rate);
        let capacity =
            ((input.len() as f64 * ratio).ceil() as usize).saturating_add(inner.output_capacity);
        let mut output = Vec::with_capacity(capacity);
        while self.pending.len() >= inner.chunk_size {
            self.input.clear();
            self.input.extend(self.pending.drain(..inner.chunk_size));
            let input_adapter = InterleavedSlice::new(&self.input, 1, inner.chunk_size)
                .map_err(|error| ResampleError::Process(error.to_string()))?;
            let mut output_adapter =
                InterleavedSlice::new_mut(&mut self.output, 1, inner.output_capacity)
                    .map_err(|error| ResampleError::Process(error.to_string()))?;
            let (_, produced) = inner
                .resampler
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .map_err(|error| ResampleError::Process(error.to_string()))?;
            let skip = self.delay_remaining.min(produced);
            self.delay_remaining -= skip;
            output.extend_from_slice(&self.output[skip..produced]);
        }
        Ok(output)
    }
}

#[derive(Debug, Error)]
pub enum ResampleError {
    #[error("resampler configuration failed: {0}")]
    Config(String),

    #[error("resampling failed: {0}")]
    Process(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_keeps_samples() {
        let mut resampler = Resampler::new(48_000, 48_000).expect("resampler");
        assert_eq!(
            resampler.process(&[0.1, 0.2, 0.3]).expect("process"),
            vec![0.1, 0.2, 0.3]
        );
    }

    #[test]
    fn rejects_zero_sample_rates() {
        assert!(matches!(
            Resampler::new(0, 48_000),
            Err(ResampleError::Config(_))
        ));
    }

    #[test]
    fn downsampling_preserves_duration() {
        let mut resampler = Resampler::new(48_000, 16_000).expect("resampler");
        let output = resampler
            .process(&vec![0.5; 48_000])
            .expect("process")
            .len();
        assert!((15_900..=16_100).contains(&output), "{output}");
    }

    #[test]
    fn upsampling_preserves_duration() {
        let mut one_second = Resampler::new(16_000, 48_000).expect("resampler");
        let first = one_second
            .process(&vec![0.5; 16_000])
            .expect("process")
            .len();
        let mut two_seconds = Resampler::new(16_000, 48_000).expect("resampler");
        let second = two_seconds
            .process(&vec![0.5; 32_000])
            .expect("process")
            .len();
        let extra = second.saturating_sub(first);
        assert!((47_900..=48_100).contains(&extra), "{extra}");
    }
}

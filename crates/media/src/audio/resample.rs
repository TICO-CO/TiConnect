#[derive(Debug, Clone)]
pub struct Resampler {
    input_rate: u32,
    output_rate: u32,
    position: f64,
    previous: f32,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate: input_rate.max(1),
            output_rate: output_rate.max(1),
            position: 0.0,
            previous: 0.0,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.input_rate == self.output_rate {
            return input.to_vec();
        }
        let step = self.input_rate as f64 / self.output_rate as f64;
        let mut output = Vec::with_capacity((input.len() as f64 / step).ceil() as usize + 1);
        let mut index = self.position;
        while index < input.len() as f64 {
            let base = index.floor() as usize;
            let fraction = (index - base as f64) as f32;
            let current = input[base];
            let next = input.get(base + 1).copied().unwrap_or(current);
            let sample = current + (next - current) * fraction;
            output.push(sample);
            index += step;
        }
        self.position = index - input.len() as f64;
        if let Some(last) = input.last() {
            self.previous = *last;
        }
        let _ = self.previous;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_keeps_samples() {
        let mut resampler = Resampler::new(48_000, 48_000);
        assert_eq!(resampler.process(&[0.1, 0.2, 0.3]), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn downsampling_reduces_length() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let output = resampler.process(&vec![0.5; 480]);
        assert!((150..=162).contains(&output.len()), "{}", output.len());
    }

    #[test]
    fn upsampling_increases_length() {
        let mut resampler = Resampler::new(16_000, 48_000);
        let output = resampler.process(&vec![0.5; 160]);
        assert!((470..=490).contains(&output.len()), "{}", output.len());
    }
}

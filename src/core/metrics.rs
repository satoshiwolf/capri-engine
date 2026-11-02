use std::collections::VecDeque;

/// Rolling frame timing statistics. Values are durations in seconds.
#[derive(Debug, Clone)]
pub struct FrameMetrics {
    capacity: usize,
    samples: VecDeque<f64>,
}

impl FrameMetrics {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            samples: VecDeque::with_capacity(capacity.max(1)),
        }
    }

    pub fn record(&mut self, duration_secs: f64) -> bool {
        if !duration_secs.is_finite() || duration_secs <= 0.0 {
            return false;
        }
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(duration_secs);
        true
    }

    pub fn average_secs(&self) -> Option<f64> {
        (!self.samples.is_empty())
            .then(|| self.samples.iter().sum::<f64>() / self.samples.len() as f64)
    }

    pub fn percentile_secs(&self, percentile: f64) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let percentile = percentile.clamp(0.0, 100.0);
        let mut sorted = self.samples.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(f64::total_cmp);
        let rank = ((percentile / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted.get(rank).copied()
    }

    /// Returns the population standard deviation between consecutive frame durations.
    pub fn jitter_secs(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let deltas = self
            .samples
            .iter()
            .zip(self.samples.iter().skip(1))
            .map(|(previous, current)| current - previous)
            .collect::<Vec<_>>();
        let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
        let variance = deltas
            .iter()
            .map(|delta| (delta - mean).powi(2))
            .sum::<f64>()
            / deltas.len() as f64;
        Some(variance.sqrt())
    }

    pub fn max_secs(&self) -> Option<f64> {
        self.samples.iter().copied().max_by(f64::total_cmp)
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9);
    }

    #[test]
    fn metrics_reject_invalid_and_bound_old_samples() {
        let mut metrics = FrameMetrics::new(2);

        assert!(!metrics.record(0.0));
        assert!(!metrics.record(f64::NAN));
        assert!(metrics.record(0.010));
        assert!(metrics.record(0.020));
        assert!(metrics.record(0.030));

        assert_eq!(metrics.len(), 2);
        assert_close(metrics.average_secs().expect("average"), 0.025);
        assert_close(metrics.max_secs().expect("maximum"), 0.030);
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let mut metrics = FrameMetrics::new(5);
        for value in [0.010, 0.020, 0.030, 0.040, 0.050] {
            assert!(metrics.record(value));
        }

        assert_close(metrics.percentile_secs(50.0).expect("median"), 0.030);
        assert_close(metrics.percentile_secs(95.0).expect("p95"), 0.050);
    }

    #[test]
    fn jitter_is_zero_for_constant_frames_and_positive_for_variable_frames() {
        let mut stable = FrameMetrics::new(8);
        for _ in 0..8 {
            assert!(stable.record(1.0 / 60.0));
        }
        assert_close(stable.jitter_secs().expect("stable jitter"), 0.0);

        let mut variable = FrameMetrics::new(8);
        for value in [0.010, 0.020, 0.010, 0.030] {
            assert!(variable.record(value));
        }
        assert!(variable.jitter_secs().expect("variable jitter") > 0.0);
    }
}

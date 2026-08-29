use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Sample {
    pub rt: Duration,
    pub status: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    samples: Vec<Sample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub total: Duration,
    pub slowest: Duration,
    pub fastest: Duration,
    pub average: Duration,
    pub rps: f64,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistBin {
    pub start: Duration,
    pub end: Duration,
    pub count: u64,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, rt: Duration, status: i32) {
        self.samples.push(Sample { rt, status });
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn summary(&self, wall: Duration) -> Summary {
        if self.samples.is_empty() {
            return Summary {
                total: wall,
                slowest: Duration::ZERO,
                fastest: Duration::ZERO,
                average: Duration::ZERO,
                rps: 0.0,
                count: 0,
            };
        }
        let count = self.samples.len() as u64;
        let mut slowest = Duration::ZERO;
        let mut fastest = Duration::MAX;
        let mut sum = Duration::ZERO;
        for s in &self.samples {
            if s.rt > slowest {
                slowest = s.rt;
            }
            if s.rt < fastest {
                fastest = s.rt;
            }
            sum += s.rt;
        }
        let average = sum / count as u32;
        let rps = if wall.is_zero() {
            0.0
        } else {
            count as f64 / wall.as_secs_f64()
        };
        Summary {
            total: wall,
            slowest,
            fastest,
            average,
            rps,
            count,
        }
    }

    /// Nearest-rank: index = ceil(p * n).saturating_sub(1).min(n-1). Empty → None.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let mut rts: Vec<Duration> = self.samples.iter().map(|s| s.rt).collect();
        rts.sort();
        let n = rts.len();
        let idx = ((p * n as f64).ceil() as usize)
            .saturating_sub(1)
            .min(n - 1);
        Some(rts[idx])
    }

    pub fn histogram(&self, buckets: usize) -> Vec<HistBin> {
        if self.samples.is_empty() || buckets == 0 {
            return Vec::new();
        }
        let mut min = Duration::MAX;
        let mut max = Duration::ZERO;
        for s in &self.samples {
            if s.rt < min {
                min = s.rt;
            }
            if s.rt > max {
                max = s.rt;
            }
        }
        if min == max {
            return vec![HistBin {
                start: min,
                end: max,
                count: self.samples.len() as u64,
            }];
        }
        let span = max.saturating_sub(min);
        let width = span / buckets as u32;
        let mut bins: Vec<HistBin> = (0..buckets)
            .map(|i| {
                let start = min + width * i as u32;
                let end = if i + 1 == buckets {
                    max
                } else {
                    min + width * (i as u32 + 1)
                };
                HistBin {
                    start,
                    end,
                    count: 0,
                }
            })
            .collect();
        for s in &self.samples {
            let mut i = if width.is_zero() {
                0
            } else {
                s.rt.saturating_sub(min).as_nanos() / width.as_nanos().max(1)
            } as usize;
            if i >= buckets {
                i = buckets - 1;
            }
            bins[i].count += 1;
        }
        bins
    }

    pub fn status_counts(&self) -> BTreeMap<i32, u64> {
        let mut m = BTreeMap::new();
        for s in &self.samples {
            *m.entry(s.status).or_insert(0) += 1;
        }
        m
    }

    pub fn render(&self, wall: Duration) -> String {
        let sum = self.summary(wall);
        let mut out = String::new();
        out.push_str("Summary:\n");
        out.push_str(&format!("  Total:\t{:.4} secs\n", sum.total.as_secs_f64()));
        out.push_str(&format!(
            "  Slowest:\t{:.4} secs\n",
            sum.slowest.as_secs_f64()
        ));
        out.push_str(&format!(
            "  Fastest:\t{:.4} secs\n",
            sum.fastest.as_secs_f64()
        ));
        out.push_str(&format!(
            "  Average:\t{:.4} secs\n",
            sum.average.as_secs_f64()
        ));
        out.push_str(&format!("  Requests/sec:\t{:.4}\n\n", sum.rps));

        out.push_str("Response time histogram:\n");
        let bins = self.histogram(5);
        let max_count = bins.iter().map(|b| b.count).max().unwrap_or(0).max(1);
        for b in &bins {
            let bar_n = ((b.count as f64 / max_count as f64) * 40.0).round() as usize;
            let bar: String = "■".repeat(bar_n);
            out.push_str(&format!(
                "  {:.3} [{n}]\t|{bar}\n",
                b.start.as_secs_f64(),
                n = b.count
            ));
        }
        out.push('\n');
        out.push_str("Latency distribution:\n");
        for (label, p) in [
            ("10%", 0.10),
            ("50%", 0.50),
            ("75%", 0.75),
            ("90%", 0.90),
            ("99%", 0.99),
        ] {
            match self.percentile(p) {
                Some(d) => out.push_str(&format!("  {label} in {:.4} secs\n", d.as_secs_f64())),
                None => out.push_str(&format!("  {label} in 0.0000 secs\n")),
            }
        }
        out.push('\n');
        out.push_str("Status code distribution:\n");
        for (st, n) in self.status_counts() {
            out.push_str(&format!("  [{st}]\t{n} responses\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zeros() {
        let s = Stats::new();
        let sum = s.summary(Duration::from_secs(1));
        assert_eq!(sum.count, 0);
        assert_eq!(sum.rps, 0.0);
        assert_eq!(sum.average, Duration::ZERO);
        assert!(s.percentile(0.99).is_none());
        assert!(s.histogram(5).is_empty());
    }

    #[test]
    fn percentile_nearest_rank() {
        let mut s = Stats::new();
        for i in 1..=10 {
            s.record(Duration::from_millis(i * 10), 0);
        }
        assert_eq!(s.percentile(0.5), Some(Duration::from_millis(50)));
        assert_eq!(s.percentile(0.99), Some(Duration::from_millis(100)));
        assert_eq!(s.percentile(0.0), Some(Duration::from_millis(10)));
    }

    #[test]
    fn histogram_five_bins() {
        let mut s = Stats::new();
        s.record(Duration::from_millis(10), 0);
        s.record(Duration::from_millis(50), 0);
        s.record(Duration::from_millis(90), 0);
        let h = s.histogram(5);
        assert_eq!(h.len(), 5);
        assert_eq!(h.iter().map(|b| b.count).sum::<u64>(), 3);
    }

    #[test]
    fn status_counts() {
        let mut s = Stats::new();
        s.record(Duration::from_millis(1), 0);
        s.record(Duration::from_millis(1), 0);
        s.record(Duration::from_millis(1), 105);
        let m = s.status_counts();
        assert_eq!(m.get(&0).copied(), Some(2));
        assert_eq!(m.get(&105).copied(), Some(1));
    }
}

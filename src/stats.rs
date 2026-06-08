// Basic statistics helpers.

pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let sum: f64 = data.iter().sum();
    sum / (data.len() as f64)
}

pub fn stddev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let var: f64 = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / ((data.len() - 1) as f64);
    var.sqrt()
}

pub fn median(data: &mut [f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = data.len();
    if n % 2 == 1 {
        data[n / 2]
    } else {
        (data[n / 2 - 1] + data[n / 2]) / 2.0
    }
}

/// Percentile of an already sorted slice. `p` in [0, 100].
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let p = p.clamp(0.0, 100.0);
    let rank = (p / 100.0) * ((sorted.len() - 1) as f64);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = rank - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

pub fn geomean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sum_ln = 0.0f64;
    let mut count = 0usize;
    for &v in data {
        if v > 0.0 {
            sum_ln += v.ln();
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    (sum_ln / (count as f64)).exp()
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct WilsonCI {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Rate {
    pub successes: u32,
    pub n: u32,
    pub p: f64,
    pub wilson_95: WilsonCI,
}

pub fn wilson_interval(successes: u32, n: u32, z: f64) -> WilsonCI {
    if n == 0 {
        return WilsonCI { low: 0.0, high: 0.0 };
    }
    let n_f = n as f64;
    let s_f = successes as f64;
    let phat = s_f / n_f;

    let z2 = z * z;
    let denom = 1.0 + z2 / n_f;
    let center = (phat + z2 / (2.0 * n_f)) / denom;
    let half = (z
        * ((phat * (1.0 - phat) + z2 / (4.0 * n_f)) / n_f).sqrt())
        / denom;

    WilsonCI {
        low: (center - half).max(0.0),
        high: (center + half).min(1.0),
    }
}

pub fn pass_rate(successes: u32, n: u32) -> Rate {
    let p = if n == 0 { 0.0 } else { successes as f64 / n as f64 };
    Rate {
        successes,
        n,
        p,
        wilson_95: wilson_interval(successes, n, 1.96),
    }
}

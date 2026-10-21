//! Touch calibration: raw XPT2046 readings -> screen pixels with an affine fit
//! (which also corrects the panel's skew and rotation), ported from touch.py.
//!
//! The calibration is [mu, mv, fx0, fx1, fx2, fy0, fy1, fy2]: raw (u, v) is centred
//! on (mu, mv) and scaled to about ±2 so single-precision floats keep their accuracy,
//! then x = fx0*u + fx1*v + fx2 and y likewise.

pub type Calib = [f32; 8];

/// A fit from a calibration of this panel, until it's calibrated on the device.
pub const DEFAULT: Calib = [1934.88892, 2092.5556, -7.2096952, 143.83858, 240.000032, -102.32201, 4.347341, 160.0];

pub const W: i32 = 480;
pub const H: i32 = 320;

pub fn map(c: &Calib, (rx, ry): (u16, u16)) -> (i32, i32) {
    let u = (rx as f32 - c[0]) / 1000.0;
    let v = (ry as f32 - c[1]) / 1000.0;
    let x = c[2] * u + c[3] * v + c[4];
    let y = c[5] * u + c[6] * v + c[7];
    ((x as i32).clamp(0, W - 1), (y as i32).clamp(0, H - 1))
}

fn det(a: [[f32; 3]; 3]) -> f32 {
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

/// Cramer's rule for a 3x3 system.
fn solve3(m: [[f32; 3]; 3], rhs: [f32; 3]) -> [f32; 3] {
    let d = det(m);
    let d = if d == 0.0 { 1e-9 } else { d };
    let mut out = [0.0; 3];
    for (col, o) in out.iter_mut().enumerate() {
        let mut mc = m;
        for i in 0..3 {
            mc[i][col] = rhs[i];
        }
        *o = det(mc) / d;
    }
    out
}

fn sqrt(v: f32) -> f32 {
    // Newton's method: enough for an error figure
    if v <= 0.0 {
        return 0.0;
    }
    let mut x = v.max(1.0);
    for _ in 0..20 {
        x = 0.5 * (x + v / x);
    }
    x
}

/// Least-squares fit of raw readings to screen targets -> (calibration, mean error px).
pub fn fit(raws: &[(u16, u16)], targets: &[(i32, i32)]) -> (Calib, f32) {
    let n = raws.len() as f32;
    let mu = raws.iter().map(|r| r.0 as f32).sum::<f32>() / n;
    let mv = raws.iter().map(|r| r.1 as f32).sum::<f32>() / n;
    let pts: alloc::vec::Vec<(f32, f32)> =
        raws.iter().map(|r| ((r.0 as f32 - mu) / 1000.0, (r.1 as f32 - mv) / 1000.0)).collect();
    let (mut suu, mut suv, mut svv, mut su, mut sv) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(u, v) in &pts {
        suu += u * u;
        suv += u * v;
        svv += v * v;
        su += u;
        sv += v;
    }
    let m = [[suu, suv, su], [suv, svv, sv], [su, sv, n]];
    let mut fits = [[0.0; 3]; 2];
    for (axis, fit) in fits.iter_mut().enumerate() {
        let t = |i: usize| if axis == 0 { targets[i].0 as f32 } else { targets[i].1 as f32 };
        let mut rhs = [0.0; 3];
        for (i, &(u, v)) in pts.iter().enumerate() {
            rhs[0] += u * t(i);
            rhs[1] += v * t(i);
            rhs[2] += t(i);
        }
        *fit = solve3(m, rhs);
    }
    let cal = [mu, mv, fits[0][0], fits[0][1], fits[0][2], fits[1][0], fits[1][1], fits[1][2]];
    let mut err = 0.0;
    for (r, &(tx, ty)) in raws.iter().zip(targets) {
        let u = (r.0 as f32 - mu) / 1000.0;
        let v = (r.1 as f32 - mv) / 1000.0;
        let x = cal[2] * u + cal[3] * v + cal[4];
        let y = cal[5] * u + cal[6] * v + cal[7];
        err += sqrt((x - tx as f32) * (x - tx as f32) + (y - ty as f32) * (y - ty as f32));
    }
    (cal, err / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_recovers_a_mapping() {
        let targets: alloc::vec::Vec<(i32, i32)> = [45, 245, 445]
            .iter()
            .flat_map(|&y| [25, 245, 455].map(move |x| (x, y % 320)))
            .collect();
        // Invert DEFAULT by sampling: find raws that map near each target
        let raws: alloc::vec::Vec<(u16, u16)> = targets
            .iter()
            .map(|&(tx, ty)| {
                let mut best = (0u16, 0u16, i32::MAX);
                for rx in (200..3900).step_by(10) {
                    for ry in (200..3900).step_by(10) {
                        let (x, y) = map(&DEFAULT, (rx, ry));
                        let d = (x - tx).abs() + (y - ty).abs();
                        if d < best.2 {
                            best = (rx, ry, d);
                        }
                    }
                }
                (best.0, best.1)
            })
            .collect();
        let (_, err) = fit(&raws, &targets);
        assert!(err < 3.0, "mean error {err}");
    }
}

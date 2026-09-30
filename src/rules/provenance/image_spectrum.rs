use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::image::{luma_crop, CROP};
use crate::lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP049",
    name: "Image spectrum has an upsampling ridge",
    tier: Tier::C,
    langs: lang::IMAGE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: false,
    path_gated: false,
    check,
};

/// Ridge prominence (log10 power above the neighbouring radii) at or above which the spectrum is
/// flagged. See the SLOP049 rows in `bench/candidates.toml` for the measurement.
const RIDGE_THRESHOLD: f32 = 0.4;

/// Radii below this are scene content, not upsampling artifacts.
const HIGH_BAND_START: usize = CROP / 4;

/// log10 power below the 8-bit quantisation noise of a windowed crop; clamping stops float
/// rounding noise in a flat crop from reading as a ridge.
const NOISE_FLOOR: f32 = 1.0;

/// Half-width of the neighbourhood a ridge must stand out from.
const NEIGHBOURHOOD: usize = 4;

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * std::f32::consts::PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Power spectrum of a Hann-windowed `CROP`x`CROP` crop, row-major, DC at index 0.
fn power_spectrum(crop: &[f32]) -> Vec<f32> {
    let hann: Vec<f32> = (0..CROP)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / CROP as f32).cos())
        .collect();
    let mut re = vec![0.0; CROP * CROP];
    let mut im = vec![0.0; CROP * CROP];
    for y in 0..CROP {
        for x in 0..CROP {
            re[y * CROP + x] = crop[y * CROP + x] * hann[y] * hann[x];
        }
        fft(&mut re[y * CROP..][..CROP], &mut im[y * CROP..][..CROP]);
    }
    let (mut col_re, mut col_im) = (vec![0.0; CROP], vec![0.0; CROP]);
    for x in 0..CROP {
        for y in 0..CROP {
            col_re[y] = re[y * CROP + x];
            col_im[y] = im[y * CROP + x];
        }
        fft(&mut col_re, &mut col_im);
        for y in 0..CROP {
            re[y * CROP + x] = col_re[y];
            im[y * CROP + x] = col_im[y];
        }
    }
    re.iter().zip(&im).map(|(r, i)| r * r + i * i).collect()
}

/// log10 of the mean power (a log-domain mean would bury a narrow peak) at each integer radius 0..CROP/2 from the spectrum's DC corner.
fn azimuthal_average(power: &[f32]) -> Vec<f32> {
    let radii = CROP / 2;
    let (mut sum, mut count) = (vec![0.0f32; radii], vec![0u32; radii]);
    for y in 0..CROP {
        for x in 0..CROP {
            let (fy, fx) = (y.min(CROP - y) as f32, x.min(CROP - x) as f32);
            let r = (fy * fy + fx * fx).sqrt().round() as usize;
            if r < radii {
                sum[r] += power[y * CROP + x];
                count[r] += 1;
            }
        }
    }
    sum.iter()
        .zip(&count)
        .map(|(s, &c)| (s / c.max(1) as f32).log10().max(NOISE_FLOOR))
        .collect()
}

/// Largest amount, in log10 power, by which any high-band radius exceeds the mean of its
/// neighbours on both sides.
fn ridge_prominence(profile: &[f32]) -> f32 {
    (HIGH_BAND_START..profile.len() - NEIGHBOURHOOD)
        .map(|r| {
            let around: Vec<f32> = (r - NEIGHBOURHOOD..=r + NEIGHBOURHOOD)
                .filter(|&i| i != r)
                .map(|i| profile[i])
                .collect();
            profile[r] - around.iter().sum::<f32>() / around.len() as f32
        })
        .fold(f32::MIN, f32::max)
}

fn ridge_of(crop: &[f32]) -> f32 {
    ridge_prominence(&azimuthal_average(&power_spectrum(crop)))
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.image else { return };
    let Some(crop) = luma_crop(&doc.bytes, doc.format) else {
        return;
    };
    let ridge = ridge_of(&crop);
    if ridge >= RIDGE_THRESHOLD {
        out.push(Diagnostic::at(
            rule,
            ctx,
            1,
            1,
            format!(
                "the {CROP}x{CROP} centre crop's high-frequency ridge prominence is {ridge:.3} log10 (threshold {RIDGE_THRESHOLD})"
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crop_from(f: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        (0..CROP * CROP).map(|i| f(i / CROP, i % CROP)).collect()
    }

    fn noise(y: usize, x: usize) -> f32 {
        let h = ((y * 73856093) ^ (x * 19349663)).wrapping_mul(2654435761) >> 8;
        (h % 256) as f32
    }

    #[test]
    fn fft_of_a_cosine_peaks_at_its_frequency() {
        let mut re: Vec<f32> = (0..CROP)
            .map(|i| (2.0 * std::f32::consts::PI * 5.0 * i as f32 / CROP as f32).cos())
            .collect();
        let mut im = vec![0.0; CROP];
        fft(&mut re, &mut im);
        let mag: Vec<f32> = re.iter().zip(&im).map(|(r, i)| r * r + i * i).collect();
        assert!(mag[5] > 1e4 && mag[7] < 1e-2);
    }

    #[test]
    fn periodic_grid_has_a_larger_ridge_than_noise() {
        let grid =
            crop_from(|y, x| noise(y, x) * 0.2 + if x % 4 == 0 || y % 4 == 0 { 60.0 } else { 0.0 });
        let plain = crop_from(|y, x| noise(y, x) * 0.2);
        assert!(ridge_of(&grid) > ridge_of(&plain) + 0.5);
    }

    #[test]
    fn flat_crop_has_no_ridge() {
        assert_eq!(ridge_of(&crop_from(|_, _| 128.0)), 0.0);
    }

    #[test]
    fn oversize_declared_dimensions_decode_to_none() {
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&40000u32.to_be_bytes());
        png.extend_from_slice(&40000u32.to_be_bytes());
        png.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        assert!(luma_crop(&png, crate::image::ImageFormat::Png).is_none());
    }
}

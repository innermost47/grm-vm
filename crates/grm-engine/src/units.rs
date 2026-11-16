// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The machine's own units: inside, it counts where c = hbar = G = kB = 1 ("ticks", Planck units, as pygrm.units).
//! A node with a dimension [M L T Q K] is given and read in SI; this is the log10 of its unit.

// CODATA 2018
const C: f64 = 299792458.0;
const HBAR: f64 = 1.054571817e-34;
const E: f64 = 1.602176634e-19;
const KB: f64 = 1.380649e-23;
const G: f64 = 6.67430e-11;
const ALPHA: f64 = 7.2973525693e-3;

/// The SI value of the unit of each base dimension (M, L, T, Q, K) in Planck units.
fn planck_units() -> [f64; 5] {
    let mp = (HBAR * C / G).sqrt();
    let tp = (HBAR * G / C.powi(5)).sqrt();
    [mp, C * tp, tp, E / ALPHA.sqrt(), mp * C * C / KB]
}

/// log10 of the SI value of "1" of a quantity of this dimension, inside the machine.
pub fn unit_log10(dim: &[f64; 5]) -> f64 {
    planck_units().iter().zip(dim.iter()).filter(|(_, p)| **p != 0.0).map(|(u, p)| p * u.log10()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_planck_time_is_one_tick() {
        // a time unit is the Planck time, 5.39e-44 s
        let t = unit_log10(&[0.0, 0.0, 1.0, 0.0, 0.0]);
        assert!((10f64.powf(t) / 5.391247e-44 - 1.0).abs() < 1e-5);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! The world, as Our World in Data publishes it: a few indicators chosen by category (health, people, economy,
//! energy, climate, knowledge), each a chart of theirs (by its slug), each category a network of its own. The
//! categories are a first frame: what ties them is for the machine to find, not drawn here.

use indexmap::IndexMap;
use serde_json::Value;

use grm_engine::error::{GrmError, Result};

/// An indicator: its category, the chart's slug, the start of its column's short name, the node it becomes, and
/// what it is (in English, in French).
#[derive(Debug, Clone, Copy)]
pub struct Indicator {
    pub category: &'static str,
    pub slug: &'static str,
    pub column: &'static str,
    pub node: &'static str,
    pub en: &'static str,
    pub fr: &'static str,
}

/// The categories, with their words (English, French).
pub const CATEGORIES: &[(&str, &str, &str)] = &[
    ("health", "Health", "Santé"),
    ("people", "People", "Population"),
    ("economy", "Economy", "Économie"),
    ("energy", "Energy", "Énergie"),
    ("climate", "Climate", "Climat"),
    ("knowledge", "Knowledge", "Savoir"),
];

/// The indicators asked by default (their slugs and columns checked against Our World in Data's metadata).
pub const INDICATORS: &[Indicator] = &[
    Indicator {
        category: "health",
        slug: "life-expectancy",
        column: "life_expectancy",
        node: "life_expectancy",
        en: "life expectancy at birth (years)",
        fr: "espérance de vie à la naissance (années)",
    },
    Indicator {
        category: "health",
        slug: "child-mortality",
        column: "child_mortality",
        node: "child_mortality",
        en: "children dying before 5 (per 100 born)",
        fr: "enfants morts avant 5 ans (pour 100 naissances)",
    },
    Indicator {
        category: "people",
        slug: "population",
        column: "population",
        node: "population",
        en: "population (people)",
        fr: "population (personnes)",
    },
    Indicator {
        category: "people",
        slug: "children-per-woman-un",
        column: "fertility_rate",
        node: "fertility",
        en: "births per woman",
        fr: "naissances par femme",
    },
    Indicator {
        category: "people",
        slug: "share-of-population-urban",
        column: "share__area_type_urban",
        node: "urban_share",
        en: "share living in towns (%)",
        fr: "part vivant en ville (%)",
    },
    Indicator {
        category: "economy",
        slug: "gdp-per-capita-worldbank",
        column: "ny_gdp_pcap",
        node: "gdp_per_capita",
        en: "GDP per person (international $ of 2021)",
        fr: "PIB par personne ($ internationaux de 2021)",
    },
    Indicator {
        category: "economy",
        slug: "share-of-population-in-extreme-poverty",
        column: "headcount_ratio",
        node: "extreme_poverty",
        en: "share living on less than $3 a day (%)",
        fr: "part vivant avec moins de 3 $ par jour (%)",
    },
    Indicator {
        category: "energy",
        slug: "per-capita-energy-use",
        column: "total_energy_supply_per_capita",
        node: "energy_per_capita",
        en: "primary energy per person (kWh)",
        fr: "énergie primaire par personne (kWh)",
    },
    Indicator {
        category: "climate",
        slug: "co2-emissions-per-capita",
        column: "emissions_total_per_capita",
        node: "co2_per_capita",
        en: "CO2 emitted per person (tonnes)",
        fr: "CO2 émis par personne (tonnes)",
    },
    Indicator {
        category: "climate",
        slug: "temperature-anomaly",
        column: "near_surface_temperature_anomaly",
        node: "temperature_anomaly",
        en: "warming since 1861-1890 (°C, the World only)",
        fr: "réchauffement depuis 1861-1890 (°C, le monde seulement)",
    },
    Indicator {
        category: "knowledge",
        slug: "mean-years-of-schooling-long-run",
        column: "mf_youth_and_adults",
        node: "schooling_years",
        en: "years at school (15 to 64 years old)",
        fr: "années d'école (15 à 64 ans)",
    },
    Indicator {
        category: "knowledge",
        slug: "share-of-individuals-using-the-internet",
        column: "it_net_user",
        node: "internet_share",
        en: "share using the Internet (%)",
        fr: "part utilisant Internet (%)",
    },
];

/// A category's workshop name.
pub fn workshop_of(category: &str) -> String {
    format!("world_{category}")
}

/// One indicator for one entity (a country, a region, "World"): (year, value), oldest first. The column is the first
/// whose short name starts as the indicator says (the exact names change with their versions).
pub fn series(csv: &str, entity: &str, column: &str) -> Result<Vec<(f64, f64)>> {
    let records = crate::csv_records(csv);
    let rows: Vec<&Value> = records
        .as_array()
        // the headers came capitalised once ("Entity", "Year"), in small letters now ("entity", "year"): both read
        .map(|a| {
            a.iter()
                .filter(|r| {
                    r.get("Entity")
                        .or_else(|| r.get("entity"))
                        .and_then(|e| e.as_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case(entity))
                })
                .collect()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        return Err(GrmError::new(format!("no row for {entity}")));
    }
    let key = rows[0]
        .as_object()
        .and_then(|o| o.keys().find(|k| k.starts_with(column)).cloned())
        .ok_or_else(|| GrmError::new(format!("no column {column}…")))?;
    let mut out: Vec<(f64, f64)> = rows
        .iter()
        .filter_map(|r| {
            let year = r
                .get("Year")
                .or_else(|| r.get("year"))
                .and_then(|y| y.as_f64().or_else(|| y.as_str().and_then(|s| s.parse().ok())))?;
            let v = r.get(&key).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))?;
            v.is_finite().then_some((year, v))
        })
        .collect();
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

/// Several series put together year by year: the rows a workshop is fed (a value missing a year is no measurement).
pub fn rows(series: &IndexMap<String, Vec<(f64, f64)>>) -> Vec<(f64, IndexMap<String, f64>)> {
    let mut years: Vec<f64> = series.values().flat_map(|s| s.iter().map(|p| p.0)).collect();
    years.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    years.dedup();
    years
        .into_iter()
        .map(|y| {
            let at: IndexMap<String, f64> =
                series.iter().filter_map(|(n, s)| s.iter().find(|p| p.0 == y).map(|p| (n.clone(), p.1))).collect();
            (y, at)
        })
        .filter(|(_, at)| !at.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chart_of_our_world_in_data_becomes_rows_by_year() {
        let le = "entity,code,year,life_expectancy_0\nFrance,FRA,1950,66.5\nFrance,FRA,1960,70.2\nSpain,ESP,1950,60\n";
        let cm = "Entity,Code,Year,child_mortality_rate,owid_region\nFrance,FRA,1960,3.1,Europe\nFrance,FRA,1950,5.2,Europe\n";
        let mut s = IndexMap::new();
        s.insert("life_expectancy".to_string(), series(le, "France", "life_expectancy").unwrap());
        s.insert("child_mortality".to_string(), series(cm, "France", "child_mortality").unwrap());
        let r = rows(&s);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, 1950.0);
        assert_eq!(r[0].1["child_mortality"], 5.2);
        assert_eq!(r[1].1["life_expectancy"], 70.2);
        assert!(series(le, "Peru", "life_expectancy").is_err());
        assert!(INDICATORS.iter().all(|i| CATEGORIES.iter().any(|c| c.0 == i.category)));
    }
}

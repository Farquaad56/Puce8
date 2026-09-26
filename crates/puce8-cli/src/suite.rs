use std::fs;
use std::path::Path;

use crate::die;
use crate::harness;
use crate::parse_next;
use puce8_core::nes::Nes;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct RomEntry {
    chemin: String,
    protocole: String,
    phase: String,
    etape: String,
    #[serde(default)]
    max_frames: Option<u32>,
    #[serde(default)]
    optionnel: bool,
    #[serde(default)]
    attendu: Option<String>,
    #[serde(default)]
    hash_attendu: Option<String>,
    #[serde(default)]
    frames: Option<u32>,
    /// Adresse du resultat pour le protocole blarggF8 (defaut $F8).
    #[serde(default)]
    result_addr: Option<u16>,
}

#[derive(Debug)]
struct SuiteResult {
    rom: String,
    protocole: String,
    resultat: String,
    detail: String,
    obligatoire: bool,
}

fn parse_etape_num(etape: &str) -> u16 {
    let s = etape.trim();
    if let Some(n) = s.strip_prefix('E').or_else(|| s.strip_prefix('e')) {
        n.parse::<u16>().unwrap_or(0)
    } else {
        0
    }
}

fn run_entry(entry: &RomEntry, roms_dir: &Path) -> SuiteResult {
    let full_path = roms_dir.join(&entry.chemin);
    if !full_path.exists() {
        return SuiteResult {
            rom: entry.chemin.clone(),
            protocole: entry.protocole.clone(),
            resultat: "ERREUR_ROM".to_string(),
            detail: format!("file not found: {}", full_path.display()),
            obligatoire: false,
        };
    }

    let bytes = match fs::read(&full_path) {
        Ok(b) => b,
        Err(e) => {
            return SuiteResult {
                rom: entry.chemin.clone(),
                protocole: entry.protocole.clone(),
                resultat: "ERREUR_ROM".to_string(),
                detail: format!("read error: {}", e),
                obligatoire: false,
            };
        }
    };

    let mut nes = match Nes::from_rom(&bytes) {
        Ok(n) => n,
        Err(e) => {
            return SuiteResult {
                rom: entry.chemin.clone(),
                protocole: entry.protocole.clone(),
                resultat: "ERREUR_ROM".to_string(),
                detail: format!("bad ROM: {:?}", e),
                obligatoire: false,
            };
        }
    };

    let result = match entry.protocole.as_str() {
        "blargg6000" => {
            let max = entry.max_frames.unwrap_or(6000);
            harness::run_blargg6000(&mut nes, max)
        }
        "blarggF8" => {
            let fr = entry.frames.unwrap_or(60);
            let addr = entry.result_addr.unwrap_or(0xF8);
            harness::run_blargg_f8(&mut nes, fr, addr)
        }
        "image" => {
            let fr = entry.frames.unwrap_or(35);
            harness::run_image(
                &mut nes,
                fr,
                entry.hash_attendu.as_deref().unwrap_or("<aucun>"),
            )
        }
        _ => {
            return SuiteResult {
                rom: entry.chemin.clone(),
                protocole: entry.protocole.clone(),
                resultat: "PROTOCOLE_INCONNU".to_string(),
                detail: format!("unknown protocol: {}", entry.protocole),
                obligatoire: false,
            };
        }
    };

    let detail = if !result.texte.is_empty() {
        result.texte.clone()
    } else if result.resultat != "REUSSI" {
        format!("code={}", result.code)
    } else {
        String::new()
    };

    SuiteResult {
        rom: entry.chemin.clone(),
        protocole: entry.protocole.clone(),
        resultat: result.resultat,
        detail,
        obligatoire: false, // will be set by caller
    }
}

pub fn do_suite(args: &[String]) -> i32 {
    let roms_dir = Path::new("tests/roms");
    let catalogue_path = Path::new("tests/catalogue.toml");

    let mut phase_filter: Option<String> = None;
    let mut jusqua: Option<String> = None;
    let mut rapport_path: Option<String> = None;

    let rest = &args[2..];
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--phase" => {
                phase_filter = Some(parse_next(rest, i));
                i += 1;
            }
            "--jusqua" => {
                jusqua = Some(parse_next(rest, i));
                i += 1;
            }
            "--rapport" => {
                rapport_path = Some(parse_next(rest, i));
                if let Some(ref p) = rapport_path {
                    if !p.starts_with("out/") {
                        eprintln!("error: rapport path must start with out/ : {}", p);
                        return 2;
                    }
                }
                i += 1;
            }
            _ => {
                eprintln!("error: unknown flag: {}", rest[i]);
                return 2;
            }
        }
        i += 1;
    }

    let catalogue_content = fs::read_to_string(catalogue_path).unwrap_or_else(|e| {
        die(
            2,
            &format!("cannot read {}: {}", catalogue_path.display(), e),
        )
    });

    let value: toml::Value = toml::from_str(&catalogue_content)
        .unwrap_or_else(|e| die(2, &format!("bad catalogue.toml: {:?}", e)));

    let roms = match value.get("rom") {
        Some(toml::Value::Array(arr)) => arr,
        _ => {
            eprintln!("error: no 'rom' array in catalogue");
            return 2;
        }
    };

    let jusqua_num = match &jusqua {
        Some(j) => parse_etape_num(j),
        None => u16::MAX,
    };

    let mut results: Vec<SuiteResult> = Vec::new();

    for entry_val in roms {
        if let Ok(entry) =
            serde_json::from_str::<RomEntry>(&serde_json::to_string(entry_val).unwrap_or_default())
        {
            // Filter by phase
            if let Some(ref pf) = phase_filter {
                if entry.phase != *pf {
                    continue;
                }
            }

            // Filter by etape <= jusqua
            let etape_num = parse_etape_num(&entry.etape);
            if etape_num > jusqua_num {
                continue;
            }

            // Skip nestest (already covered by cargo test)
            if entry.protocole == "nestest" {
                continue;
            }

            // Skip image without hash_attendu
            if entry.protocole == "image" && entry.hash_attendu.is_none() {
                continue;
            }

            let obligatoire = !entry.optionnel && entry.attendu.as_deref() != Some("echec");

            let mut res = run_entry(&entry, roms_dir);
            res.obligatoire = obligatoire;
            results.push(res);
        }
    }

    // Compute totals
    let total = results.iter().filter(|r| r.obligatoire).count();
    let failures: Vec<&SuiteResult> = results
        .iter()
        .filter(|r| r.obligatoire && r.resultat != "REUSSI")
        .collect();

    // Console output: total and failing lines only
    for res in &results {
        if !res.obligatoire {
            continue;
        }
        if res.resultat == "REUSSI" {
            continue;
        }
        if res.detail.is_empty() {
            eprintln!("ECHEC: {} [{}]", res.rom, res.protocole);
        } else {
            eprintln!("ECHEC: {} [{}] {}", res.rom, res.protocole, res.detail);
        }
    }

    if failures.is_empty() {
        println!("Total : {}/{}", total, total);
    } else {
        let success = total - failures.len();
        println!("Total : {}/{}", success, total);
    }

    // Write rapport Markdown
    if let Some(ref rp) = rapport_path {
        let mut md = String::new();
        md.push_str("# Rapport suite\n\n");
        md.push_str("| ROM | protocole | resultat | detail |\n");
        md.push_str("|---|---|---|---|\n");
        for res in &results {
            if !res.obligatoire {
                continue;
            }
            let detail = res.detail.replace(['|', '\n'], " ");
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                res.rom, res.protocole, res.resultat, detail
            ));
        }
        md.push_str(&format!("\nTotal : {}/{}\n", total - failures.len(), total));
        fs::write(rp, &md).unwrap_or_else(|e| die(2, &format!("cannot write {}: {}", rp, e)));
    }

    if failures.is_empty() {
        0
    } else {
        1
    }
}

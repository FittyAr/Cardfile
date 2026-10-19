//! Spelling dictionaries for the interface languages and their on-demand install.
//!
//! en_US and es_ES ship with every package (`scripts/fetch-dictionaries.sh`). The others
//! are downloaded into the user's data folder when asked for, from the same pinned commit
//! of `wooorm/dictionaries` and checked against the SHA-256 below, so nothing upstream
//! can change what gets installed. They are not part of Cardfile's distribution.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context as _, bail};
use sha2::{Digest, Sha256};

const REV: &str = "8cfea406b505e4d7df52d5a19bce525df98c54ab";
/// The largest file in the catalog is ~13 MB.
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;

/// A dictionary that can be downloaded: its folder in the repository and the SHA-256
/// of `index.aff`, `index.dic` and `license`.
struct Remote {
    dir: &'static str,
    aff: &'static str,
    dic: &'static str,
    license: &'static str,
}

/// Interface locale → dictionary code (the file name: `<code>.aff`/`.dic`).
/// Locales without a Hunspell dictionary (ar, bn, hi, id, ja, th, zh) are absent, and so
/// is tr: `spellbook` cannot read the Turkish dictionary.
const LOCALES: &[(&str, &str)] = &[
    ("en", "en_US"),
    ("es", "es_ES"),
    ("de", "de_DE"),
    ("fa", "fa_IR"),
    ("fr", "fr_FR"),
    ("it", "it_IT"),
    ("ko", "ko_KR"),
    ("nl", "nl_NL"),
    ("pl", "pl_PL"),
    ("pt-BR", "pt_BR"),
    ("ru", "ru_RU"),
    ("uk", "uk_UA"),
    ("vi", "vi_VN"),
];

fn remote(code: &str) -> Option<Remote> {
    let r = |dir, aff, dic, license| Remote {
        dir,
        aff,
        dic,
        license,
    };
    Some(match code {
        "de_DE" => r(
            "de",
            "57fdd1b16aac2131003c91e0cf2a488becb970382a402a9ce089307301cb3ef0",
            "b5c781a0cf6f285fb6b9b8ab02fbea104b987104a1efdda8a835837e89e3ec77",
            "03abf202c6e207d41f054083cafccbb174a30f6b330ea8dba3aeef0a279cb83e",
        ),
        "fa_IR" => r(
            "fa",
            "dba0980816354ad969aa46dda0c040eeebda4a9381be331e17eae8647a2a1e1c",
            "314f31e059994f26e8016c834b14dfd10f74834b3f623a6040265b8f2a9dddab",
            "7429736f069a84049bcfbee436e5cea3a3fd9b98348d880d9e43ebcf39721989",
        ),
        "fr_FR" => r(
            "fr",
            "05a735d34c912e4e381ff08ee7c747923ccf5cf9dca81d8467982fa1ca51c2b7",
            "984e933237bc1224a48f42828233be9b03228260ef67aa8e2bdddcd03a26230d",
            "0c785aebc7aae8aa00b8163e9b3219a8785a9900b71e85475204e48a5651ced4",
        ),
        "it_IT" => r(
            "it",
            "5770cd3e16d494c045b4a9a4a9fcd7962577e642d0384a7129c020a12cdd2c79",
            "b1348fbdb6f441ea9dd7e33b2cfcb96ead39ccd5e48bf894972774cd5aa86abb",
            "e48a7f7b3ec37ebb2c0d625528a7a677818de039bbae43e8f2c1dce6fd2a13e6",
        ),
        "ko_KR" => r(
            "ko",
            "7b8930ce1357691591a3c38afc9cba787ec6dce0e30d0438bd156c4b4c94dbdf",
            "ea275b5fe256925a95f9192a173f7c3d3c2480fa748371c64b89d7e0cc531b1d",
            "3e81dd90f6d8877ccb2ed2b262ce72ee8a6ccf3a21179a8949abf0d85ac7fda0",
        ),
        "nl_NL" => r(
            "nl",
            "b430d9cf5f4170d9e5072397927e1402b34aa877d1f4c8fbc00dab6370d87312",
            "be192fdc36efd39dde0fb9a5a113143e0a906782e81ed68a74a710edced378d0",
            "fd9e95c360245eab3c388885062820754785faa4123964994cfdecb950b71948",
        ),
        "pl_PL" => r(
            "pl",
            "de7b54a8fa37562d58b481cf3d87a63e3dcc2d74021f6403eb0e4f5df9ecc225",
            "74b6155391025419967c26913541b55b36d58e80b1028037d4a0b8ddcb18d0b7",
            "8bb92c291eb431ffe32f7c42a719a0b54acf2851690fe12ca292e248a12a37e6",
        ),
        "pt_BR" => r(
            "pt",
            "a9f3621b94eb4d6838474bf9982027758e77dcb248815f49409b1a4a477dbd27",
            "32e2edd83541d58613bcc83ef731a79aadf85ed57e977a16c6b5f711a7a36cee",
            "bb064f3db7ba5f5536da7d079773f692c5f928822e8aebc3d78259ff6cceb9e5",
        ),
        "ru_RU" => r(
            "ru",
            "38ce7d4af78e211e9bafe4bf7e3d6a2c420591136cb738ec6648f8fdf6524cd7",
            "f6047416a0204adbecf3a451b874ec8a97ee37e2cbc714466ef04d8dbcc0d6fc",
            "262af2f6ad70a61e5ee1332ff44fa8ee50edca819cf33207d8ad6ba6a0c9be52",
        ),
        "uk_UA" => r(
            "uk",
            "4243327a3aa19c6db5a4fb2e0c4e3eba6ad26e7ccb8a3f4e634c28d9a2d18956",
            "48cbe32567a2c3c98f83d437bcd84ad3c11310e9df4d6c15d888b1e67654be31",
            "d62f065830aa3739cc031156b9690805c7b2e811b4a178c8b4acd8725d561c94",
        ),
        "vi_VN" => r(
            "vi",
            "0d836ebf1d4f1e9964d931fbb76464b24037b71995c73784166a07c43e2a7ae5",
            "9c56725b43e66de33d28aac8e58124e38247c8b2be34a8469b3113169110256c",
            "4765f3b113b57fec3fff1d790128b4dca78069bebb156bf0e241a29cf4fe3984",
        ),
        _ => return None,
    })
}

/// The dictionary that matches an interface locale, if one exists.
pub fn for_locale(locale: &str) -> Option<&'static str> {
    LOCALES
        .iter()
        .find(|(l, _)| *l == locale)
        .map(|(_, code)| *code)
}

/// Whether `code` can be downloaded with [`install`].
pub fn is_downloadable(code: &str) -> bool {
    remote(code).is_some()
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(10 * 60)))
            .build()
            .into()
    })
}

fn fetch(dir: &str, file: &str, sha256: &str) -> anyhow::Result<Vec<u8>> {
    let url = format!(
        "https://raw.githubusercontent.com/wooorm/dictionaries/{REV}/dictionaries/{dir}/{file}"
    );
    let body = agent()
        .get(&url)
        .header(
            "User-Agent",
            concat!("Cardfile/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .with_context(|| format!("{dir}/{file}"))?
        .body_mut()
        .with_config()
        .limit(MAX_FILE_SIZE)
        .read_to_vec()
        .with_context(|| format!("{dir}/{file}"))?;
    let actual = Sha256::digest(&body)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            // Writing to a String cannot fail.
            let _ = write!(s, "{b:02x}");
            s
        });
    if actual != sha256 {
        bail!("checksum mismatch for {dir}/{file}");
    }
    Ok(body)
}

/// Download dictionary `code` into `dest`. Blocking: run it off the UI thread.
///
/// The `.dic` file is moved into place last, since its presence is what makes the
/// language show up as installed.
pub fn install(code: &str, dest: &Path) -> anyhow::Result<()> {
    let Some(r) = remote(code) else {
        bail!("no downloadable dictionary for {code}");
    };
    let aff = fetch(r.dir, "index.aff", r.aff)?;
    let dic = fetch(r.dir, "index.dic", r.dic)?;
    let license = fetch(r.dir, "license", r.license)?;
    std::fs::create_dir_all(dest)?;
    let place = |name: String, bytes: &[u8]| -> anyhow::Result<()> {
        let tmp = dest.join(format!(".{name}.download"));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, dest.join(&name))
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&tmp);
            })
            .with_context(|| name.clone())
    };
    place(format!("LICENSE-{code}"), &license)?;
    place(format!("{code}.aff"), &aff)?;
    place(format!("{code}.dic"), &dic)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_dictionary_is_shipped_or_downloadable() {
        for (locale, code) in LOCALES {
            let shipped = matches!(*code, "en_US" | "es_ES");
            assert!(shipped != is_downloadable(code), "{locale} → {code}");
        }
        assert_eq!(for_locale("pt-BR"), Some("pt_BR"));
        assert_eq!(for_locale("ja"), None);
    }

    #[test]
    fn locales_exist() {
        let available = cardfile_i18n::available_locales();
        for (locale, _) in LOCALES {
            assert!(available.iter().any(|l| l == locale), "{locale}");
        }
    }
}

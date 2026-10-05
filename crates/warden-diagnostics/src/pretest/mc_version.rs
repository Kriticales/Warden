//! Comparações de versões do Minecraft. Só versões finais numéricas formam uma ordem total;
//! snapshots e RC são comparadas com faixas Fabric depois de normalização local.

use std::cmp::Ordering;

/// Normaliza versões modernas como o `McVersionLookup` do Fabric Loader: snapshots viram
/// `alpha`, pré-lançamentos viram `beta` (ou `rc` até 1.16) e RC vira `rc`.
/// Para snapshots semanais o alvo vem do `release_target` do JSON oficial da Mojang.
pub(super) fn for_fabric(version: &str, release_target: Option<&str>) -> Option<String> {
    for (marker, label) in [
        ("-snapshot-", "alpha"),
        ("-rc-", "rc"),
        ("-rc", "rc"),
        ("-pre-", "beta"),
        ("-pre", "beta"),
    ] {
        if let Some((base, number)) = version.split_once(marker)
            && release_cmp(base, "1.16").is_some()
            && !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
        {
            let label = if marker.starts_with("-pre")
                && release_cmp(base, "1.16").is_some_and(Ordering::is_le)
            {
                "rc"
            } else {
                label
            };
            return Some(format!("{base}-{label}.{number}"));
        }
    }
    if let Some((year, rest)) = version.split_once('w')
        && year.len() == 2
        && year.bytes().all(|b| b.is_ascii_digit())
        && rest.len() >= 3
    {
        let (week, patch) = rest.split_at(rest.len() - 1);
        if week.bytes().all(|b| b.is_ascii_digit()) && patch.bytes().all(|b| b.is_ascii_lowercase())
        {
            let target = release_target?;
            let week: u8 = week.parse().ok()?;
            return Some(format!("{target}-alpha.{year}.{week}.{patch}"));
        }
    }
    Some(version.to_owned())
}

/// Ordena somente releases numéricos (`1.20.1`, `26.3`).
pub(super) fn release_cmp(a: &str, b: &str) -> Option<Ordering> {
    fn parts(text: &str) -> Option<Vec<u32>> {
        let parsed: Vec<u32> = text
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        (parsed.len() >= 2 && parsed.len() <= 3).then_some(parsed)
    }
    let a = parts(a)?;
    let b = parts(b)?;
    for i in 0..a.len().max(b.len()) {
        let order = a
            .get(i)
            .copied()
            .unwrap_or(0)
            .cmp(&b.get(i).copied().unwrap_or(0));
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normaliza_rc_pre_snapshot_e_preserva_26() {
        assert_eq!(
            for_fabric("1.21.1-rc1", None).as_deref(),
            Some("1.21.1-rc.1")
        );
        assert_eq!(
            for_fabric("1.21.1-pre2", None).as_deref(),
            Some("1.21.1-beta.2")
        );
        assert_eq!(for_fabric("1.16-pre2", None).as_deref(), Some("1.16-rc.2"));
        assert_eq!(
            for_fabric("26.3-snapshot-2", None).as_deref(),
            Some("26.3-alpha.2")
        );
        assert_eq!(for_fabric("26.3", None).as_deref(), Some("26.3"));
        assert_eq!(
            for_fabric("26w40a", Some("26.4")).as_deref(),
            Some("26.4-alpha.26.40.a")
        );
        assert_eq!(for_fabric("26w40a", None), None);
        assert_eq!(release_cmp("26.3", "1.21.11"), Some(Ordering::Greater));
        assert_eq!(release_cmp("1.20", "1.20.0"), Some(Ordering::Equal));
        assert_eq!(release_cmp("26w40a", "26.3"), None);
    }
}

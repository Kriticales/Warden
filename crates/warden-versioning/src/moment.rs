//! Instante das operações (data dos commits, das tags e dos pontos de segurança).
//!
//! As operações recebem o instante de fora ([`Moment`]): os testes ficam determinísticos
//! (QUALITY §4.1) e a `warden-app` informa o fuso do computador, que o Rust puro não sabe
//! descobrir sem dependência nova. Datas do calendário são calculadas aqui mesmo, com o
//! algoritmo `civil_from_days` de Howard Hinnant (domínio público).

use std::time::{SystemTime, UNIX_EPOCH};

/// Um instante com o fuso horário em que deve ser mostrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Moment {
    /// Segundos desde 1970-01-01T00:00:00Z.
    pub unix_seconds: i64,
    /// Diferença do fuso para o UTC, em minutos (`-180` no horário de Brasília).
    pub offset_minutes: i32,
}

impl Moment {
    /// Instante a partir dos segundos Unix e do fuso.
    #[must_use]
    pub const fn new(unix_seconds: i64, offset_minutes: i32) -> Self {
        Self {
            unix_seconds,
            offset_minutes,
        }
    }

    /// Agora, em UTC. A `warden-app` passa o fuso do computador com [`Moment::new`].
    #[must_use]
    pub fn now_utc() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
            });
        Self::new(seconds, 0)
    }

    pub(crate) fn from_git(time: git2::Time) -> Self {
        Self::new(time.seconds(), time.offset_minutes())
    }

    pub(crate) fn to_git(self) -> git2::Time {
        git2::Time::new(self.unix_seconds, self.offset_minutes)
    }

    /// Data e hora no fuso do instante.
    fn local_parts(self) -> Parts {
        let local = self
            .unix_seconds
            .saturating_add(i64::from(self.offset_minutes) * 60);
        let days = local.div_euclid(86_400);
        let second_of_day = local.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Parts {
            year,
            month,
            day,
            hour: second_of_day / 3600,
            minute: second_of_day % 3600 / 60,
            second: second_of_day % 60,
        }
    }

    /// Data no fuso do instante, como `2026-10-01` (cabeçalho do `CHANGELOG.md`).
    #[must_use]
    pub fn date(self) -> String {
        let p = self.local_parts();
        format!("{:04}-{:02}-{:02}", p.year, p.month, p.day)
    }

    /// Data e hora RFC 3339 no fuso do instante, como `2026-10-01T14:05:09-03:00`.
    #[must_use]
    pub fn rfc3339(self) -> String {
        let p = self.local_parts();
        let sign = if self.offset_minutes < 0 { '-' } else { '+' };
        let offset = self.offset_minutes.unsigned_abs();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{sign}{:02}:{:02}",
            p.year,
            p.month,
            p.day,
            p.hour,
            p.minute,
            p.second,
            offset / 60,
            offset % 60
        )
    }

    /// Carimbo em UTC para nomes de referência, como `20261001T170509Z`.
    #[must_use]
    pub fn compact_utc(self) -> String {
        let p = Self::new(self.unix_seconds, 0).local_parts();
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            p.year, p.month, p.day, p.hour, p.minute, p.second
        )
    }
}

struct Parts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
}

/// Dias desde 1970-01-01 → (ano, mês, dia) no calendário gregoriano proléptico.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn datas_conhecidas() {
        assert_eq!(Moment::new(0, 0).rfc3339(), "1970-01-01T00:00:00+00:00");
        // 2026-10-01T17:05:09Z
        let moment = Moment::new(1_790_874_309, 0);
        assert_eq!(moment.rfc3339(), "2026-10-01T17:05:09+00:00");
        assert_eq!(moment.compact_utc(), "20261001T170509Z");
        assert_eq!(moment.date(), "2026-10-01");
        // 2024-02-29 (bissexto) e 2000-03-01.
        assert_eq!(Moment::new(1_709_164_800, 0).date(), "2024-02-29");
        assert_eq!(Moment::new(951_868_800, 0).date(), "2000-03-01");
        assert_eq!(Moment::new(-1, 0).rfc3339(), "1969-12-31T23:59:59+00:00");
    }

    #[test]
    fn fuso_muda_a_data_local_mas_nao_o_carimbo() {
        // 2026-10-02T01:30:00Z = 2026-10-01T22:30:00-03:00
        let moment = Moment::new(1_790_904_600, -180);
        assert_eq!(moment.date(), "2026-10-01");
        assert_eq!(moment.rfc3339(), "2026-10-01T22:30:00-03:00");
        assert_eq!(moment.compact_utc(), "20261002T013000Z");
        assert_eq!(
            Moment::new(1_790_904_600, 330).rfc3339(),
            "2026-10-02T07:00:00+05:30"
        );
    }

    #[test]
    fn ida_e_volta_pelo_git() {
        let moment = Moment::new(1_790_904_600, -180);
        assert_eq!(Moment::from_git(moment.to_git()), moment);
        assert!(Moment::now_utc().unix_seconds > 1_700_000_000);
    }

    proptest! {
        /// A data calculada bate com a contagem de dias: dia seguinte = data seguinte.
        #[test]
        fn dias_consecutivos_tem_datas_consecutivas(days in -800_000i64..800_000) {
            let (y1, m1, d1) = civil_from_days(days);
            let (y2, m2, d2) = civil_from_days(days + 1);
            prop_assert!((1..=12).contains(&m1) && (1..=31).contains(&d1));
            let next_day = (y2, m2, d2) == (y1, m1, d1 + 1);
            let next_month = d2 == 1 && ((y2, m2) == (y1, m1 + 1) || (y2, m2, m1) == (y1 + 1, 1, 12));
            prop_assert!(next_day || next_month, "{y1}-{m1}-{d1} → {y2}-{m2}-{d2}");
        }
    }
}

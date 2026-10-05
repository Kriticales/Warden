//! Leitura dos cabeçalhos de limite de requisições: `Retry-After` (segundos ou data HTTP) e
//! `X-Ratelimit-Reset`/`X-Ratelimit-Remaining` do Modrinth (segundos até o fim da janela;
//! R3 §3.1).

use std::time::{Duration, SystemTime};

use reqwest::header::{HeaderMap, RETRY_AFTER};

/// `X-Ratelimit-Reset`: segundos até a janela recomeçar.
pub const RATELIMIT_RESET: &str = "x-ratelimit-reset";
/// `X-Ratelimit-Remaining`: requisições que ainda cabem na janela.
pub const RATELIMIT_REMAINING: &str = "x-ratelimit-remaining";

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok().map(str::trim)
}

/// Segundos num cabeçalho (inteiro ou decimal não negativo).
fn seconds(value: &str) -> Option<Duration> {
    if let Ok(whole) = value.parse::<u64>() {
        return Some(Duration::from_secs(whole));
    }
    let fractional = value.parse::<f64>().ok()?;
    (fractional.is_finite() && fractional >= 0.0)
        .then(|| Duration::try_from_secs_f64(fractional).ok())
        .flatten()
}

/// Espera pedida por `Retry-After`, em segundos ou como data HTTP (`now` é o relógio de
/// parede, para converter a data).
#[must_use]
pub fn retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let value = header_str(headers, RETRY_AFTER.as_str())?;
    seconds(value).or_else(|| {
        let date = httpdate::parse_http_date(value).ok()?;
        Some(date.duration_since(now).unwrap_or(Duration::ZERO))
    })
}

/// Espera até o fim da janela, por `X-Ratelimit-Reset`.
#[must_use]
pub fn ratelimit_reset(headers: &HeaderMap) -> Option<Duration> {
    header_str(headers, RATELIMIT_RESET).and_then(seconds)
}

/// Se a janela acabou (`X-Ratelimit-Remaining: 0`).
#[must_use]
pub fn ratelimit_exhausted(headers: &HeaderMap) -> bool {
    header_str(headers, RATELIMIT_REMAINING).is_some_and(|value| value == "0")
}

/// Espera que o servidor pediu: o maior entre `Retry-After` e `X-Ratelimit-Reset`.
#[must_use]
pub fn requested_wait(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    match (retry_after(headers, now), ratelimit_reset(headers)) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn retry_after_em_segundos_e_data() {
        let now = httpdate::parse_http_date("Sun, 04 Oct 2026 12:00:00 GMT").unwrap();
        assert_eq!(
            retry_after(&headers(&[("retry-after", "7")]), now),
            Some(Duration::from_secs(7))
        );
        assert_eq!(
            retry_after(
                &headers(&[("retry-after", "Sun, 04 Oct 2026 12:00:30 GMT")]),
                now
            ),
            Some(Duration::from_secs(30))
        );
        // Data no passado: não espera.
        assert_eq!(
            retry_after(
                &headers(&[("retry-after", "Sun, 04 Oct 2026 11:00:00 GMT")]),
                now
            ),
            Some(Duration::ZERO)
        );
        assert_eq!(retry_after(&headers(&[("retry-after", "logo")]), now), None);
        assert_eq!(retry_after(&headers(&[("retry-after", "-3")]), now), None);
        assert_eq!(retry_after(&HeaderMap::new(), now), None);
    }

    #[test]
    fn cabecalhos_do_modrinth() {
        let map = headers(&[
            ("x-ratelimit-reset", "12"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-limit", "300"),
        ]);
        assert_eq!(ratelimit_reset(&map), Some(Duration::from_secs(12)));
        assert!(ratelimit_exhausted(&map));
        assert!(!ratelimit_exhausted(&headers(&[(
            "x-ratelimit-remaining",
            "5"
        )])));
        assert_eq!(
            ratelimit_reset(&headers(&[("x-ratelimit-reset", "1.5")])),
            Some(Duration::from_millis(1500))
        );
        assert_eq!(
            ratelimit_reset(&headers(&[("x-ratelimit-reset", "NaN")])),
            None
        );
    }

    #[test]
    fn vale_a_maior_espera() {
        let now = SystemTime::UNIX_EPOCH;
        let map = headers(&[("retry-after", "3"), ("x-ratelimit-reset", "9")]);
        assert_eq!(requested_wait(&map, now), Some(Duration::from_secs(9)));
        let map = headers(&[("retry-after", "20"), ("x-ratelimit-reset", "9")]);
        assert_eq!(requested_wait(&map, now), Some(Duration::from_secs(20)));
        let map = headers(&[("x-ratelimit-reset", "4")]);
        assert_eq!(requested_wait(&map, now), Some(Duration::from_secs(4)));
        assert_eq!(requested_wait(&HeaderMap::new(), now), None);
    }
}

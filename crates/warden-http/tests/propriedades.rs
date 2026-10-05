//! Propriedades dos leitores de cabeçalho (entrada externa não confiável, QUALITY §9 item 6):
//! nenhum valor derruba o cliente, e as esperas lidas são coerentes.

use std::time::{Duration, SystemTime};

use proptest::prelude::*;
use warden_http::headers::{ratelimit_exhausted, ratelimit_reset, requested_wait, retry_after};
use warden_http::{HeaderMap, HeaderValue, Url, same_site};

fn headers(pairs: &[(&'static str, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        if let Ok(value) = HeaderValue::from_str(value) {
            map.insert(*name, value);
        }
    }
    map
}

proptest! {
    #[test]
    fn cabecalhos_quaisquer_nao_derrubam(
        retry in ".{0,40}",
        reset in ".{0,40}",
        remaining in ".{0,10}",
        now in 0_u64..4_000_000_000,
    ) {
        let map = headers(&[
            ("retry-after", retry),
            ("x-ratelimit-reset", reset),
            ("x-ratelimit-remaining", remaining),
        ]);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(now);
        let wait = requested_wait(&map, now);
        let parts = [retry_after(&map, now), ratelimit_reset(&map)];
        // A espera pedida é a maior das duas.
        prop_assert_eq!(wait, parts.iter().flatten().max().copied());
        let _ = ratelimit_exhausted(&map);
    }

    #[test]
    fn segundos_inteiros_sao_lidos_exatos(seconds in 0_u64..1_000_000) {
        let map = headers(&[("x-ratelimit-reset", seconds.to_string())]);
        prop_assert_eq!(ratelimit_reset(&map), Some(Duration::from_secs(seconds)));
        let map = headers(&[("retry-after", seconds.to_string())]);
        prop_assert_eq!(retry_after(&map, SystemTime::now()), Some(Duration::from_secs(seconds)));
    }

    #[test]
    fn mesmo_site_e_simetrico(a in r"[a-z]{1,8}(\.[a-z]{1,8}){0,3}", b in r"[a-z]{1,8}(\.[a-z]{1,8}){0,3}") {
        let (Ok(x), Ok(y)) = (Url::parse(&format!("https://{a}/")), Url::parse(&format!("https://{b}/"))) else {
            return Ok(());
        };
        prop_assert_eq!(same_site(&x, &y), same_site(&y, &x));
        prop_assert!(same_site(&x, &x));
    }
}

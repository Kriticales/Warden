//! Pontos de falha para os testes de injeção de falha (QUALITY §3 item 7 e §4.1).
//!
//! O código de produção chama [`check`] em pontos nomeados ("entre gravar e renomear",
//! "depois de copiar a chave"…). Sem a feature `fault-injection` (e fora dos testes desta
//! crate), [`check`] não faz nada e some na compilação. Com ela, um teste arma um ponto com
//! [`arm`] e a próxima passagem por ele devolve um erro de E/S, como se o disco ou o cofre
//! tivessem falhado ali.
//!
//! Os pontos armados valem **só para a thread do teste**: testes em paralelo não interferem
//! entre si, e o código sob teste precisa passar pelo ponto na mesma thread.

use std::io;

/// Passa por um ponto de falha. Devolve erro se um teste armou este ponto na thread atual.
#[cfg(any(test, feature = "fault-injection"))]
pub fn check(point: &'static str) -> io::Result<()> {
    armed::hit(point)
}

/// Passa por um ponto de falha. Sem a feature `fault-injection`, nunca falha.
#[cfg(not(any(test, feature = "fault-injection")))]
#[inline(always)]
pub fn check(_point: &'static str) -> io::Result<()> {
    Ok(())
}

#[cfg(any(test, feature = "fault-injection"))]
pub use armed::{FaultGuard, arm, arm_after, hits};

#[cfg(any(test, feature = "fault-injection"))]
mod armed {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::io;

    #[derive(Default)]
    struct Point {
        /// Passagens que ainda devem dar certo antes da falha; `None` = desarmado.
        fail_after: Option<usize>,
        /// Quantas vezes o ponto foi alcançado desde que foi armado.
        hits: usize,
    }

    thread_local! {
        static POINTS: RefCell<HashMap<&'static str, Point>> = RefCell::new(HashMap::new());
    }

    pub(super) fn hit(point: &'static str) -> io::Result<()> {
        POINTS.with(|points| {
            let mut points = points.borrow_mut();
            let Some(state) = points.get_mut(point) else {
                return Ok(());
            };
            state.hits += 1;
            match state.fail_after {
                Some(0) => {
                    state.fail_after = None;
                    Err(io::Error::other(format!("falha injetada em {point}")))
                }
                Some(remaining) => {
                    state.fail_after = Some(remaining - 1);
                    Ok(())
                }
                None => Ok(()),
            }
        })
    }

    /// Ponto armado; desarma ao sair de escopo.
    #[must_use = "o ponto é desarmado quando a guarda sai de escopo"]
    pub struct FaultGuard {
        point: &'static str,
    }

    impl Drop for FaultGuard {
        fn drop(&mut self) {
            POINTS.with(|points| {
                points.borrow_mut().remove(self.point);
            });
        }
    }

    /// Arma `point`: a próxima passagem por ele falha (uma vez).
    pub fn arm(point: &'static str) -> FaultGuard {
        arm_after(point, 0)
    }

    /// Arma `point` para falhar na passagem de número `successes + 1` (as `successes`
    /// primeiras dão certo).
    pub fn arm_after(point: &'static str, successes: usize) -> FaultGuard {
        POINTS.with(|points| {
            points.borrow_mut().insert(
                point,
                Point {
                    fail_after: Some(successes),
                    hits: 0,
                },
            );
        });
        FaultGuard { point }
    }

    /// Quantas vezes `point` foi alcançado nesta thread desde que foi armado.
    #[must_use]
    pub fn hits(point: &'static str) -> usize {
        POINTS.with(|points| points.borrow().get(point).map_or(0, |state| state.hits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ponto_desarmado_nao_falha() {
        assert!(check("teste.desarmado").is_ok());
        assert_eq!(hits("teste.desarmado"), 0);
    }

    #[test]
    fn falha_uma_vez_e_conta_passagens() {
        let _guard = arm("teste.uma-vez");
        let error = check("teste.uma-vez").unwrap_err();
        assert!(error.to_string().contains("teste.uma-vez"));
        assert!(check("teste.uma-vez").is_ok());
        assert_eq!(hits("teste.uma-vez"), 2);
    }

    #[test]
    fn falha_depois_de_n_sucessos() {
        let _guard = arm_after("teste.depois", 2);
        assert!(check("teste.depois").is_ok());
        assert!(check("teste.depois").is_ok());
        assert!(check("teste.depois").is_err());
        assert!(check("teste.depois").is_ok());
    }

    #[test]
    fn guarda_desarma_ao_sair() {
        {
            let _guard = arm("teste.escopo");
        }
        assert!(check("teste.escopo").is_ok());
    }

    #[test]
    fn outra_thread_nao_ve_o_ponto_armado() {
        let _guard = arm("teste.thread");
        let other = std::thread::spawn(|| check("teste.thread").is_ok())
            .join()
            .unwrap();
        assert!(other);
        assert!(check("teste.thread").is_err());
    }
}

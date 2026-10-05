//! Exclusão explícita de packs para a Lixeira do sistema.

use warden_core::PackId;

use crate::registry::Registry;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Move a pasta para a Lixeira somente após confirmar o nome do manifesto.
pub fn trash_pack(registry: &Registry, id: PackId, confirmation: &str) -> Result<()> {
    let record = registry.get(id)?;
    let pack = warden_packwiz::read_pack(&record.path)
        .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    if pack.pack.value.name != confirmation {
        return Err(Error::new(
            Code::TrashConfirmation,
            "nome de confirmação diferente",
        ));
    }
    trash::delete(&record.path).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    registry.forget(id)
}

//! Chamada mínima à AMSI (Antimalware Scan Interface) do Windows.
//! `AmsiScanBuffer` entrega os bytes ao provedor registrado (o antivírus ativo);
//! nada é gravado em disco nem enviado pela rede por este código.

use windows_sys::Win32::System::Antimalware::*;

pub struct Amsi {
    ctx: HAMSICONTEXT,
    session: HAMSISESSION,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Amsi {
    pub fn new(app: &str) -> Self {
        let name = wide(app);
        let mut ctx: HAMSICONTEXT = std::ptr::null_mut();
        let hr = unsafe { AmsiInitialize(name.as_ptr(), &mut ctx) };
        assert!(hr >= 0, "AmsiInitialize falhou: 0x{hr:08x}");
        let mut session: HAMSISESSION = std::ptr::null_mut();
        let hr = unsafe { AmsiOpenSession(ctx, &mut session) };
        assert!(hr >= 0, "AmsiOpenSession falhou: 0x{hr:08x}");
        Amsi { ctx, session }
    }

    /// Devolve (descrição do resultado, se é malware).
    pub fn scan(&self, data: &[u8], name: &str) -> (String, bool) {
        let n = wide(name);
        let mut result: AMSI_RESULT = 0;
        let hr = unsafe {
            AmsiScanBuffer(self.ctx, data.as_ptr().cast(), data.len() as u32, n.as_ptr(), self.session, &mut result)
        };
        if hr < 0 {
            return (format!("ERRO hr=0x{:08x}", hr as u32), false);
        }
        let desc = match result {
            AMSI_RESULT_CLEAN => "CLEAN(0)".to_string(),
            AMSI_RESULT_NOT_DETECTED => "NOT_DETECTED(1)".to_string(),
            r if (AMSI_RESULT_BLOCKED_BY_ADMIN_START..=AMSI_RESULT_BLOCKED_BY_ADMIN_END).contains(&r) => {
                format!("BLOCKED_BY_ADMIN({r})")
            }
            r if r >= AMSI_RESULT_DETECTED => format!("DETECTED({r})"),
            r => format!("OUTRO({r})"),
        };
        (desc, result >= AMSI_RESULT_DETECTED)
    }
}

impl Drop for Amsi {
    fn drop(&mut self) {
        unsafe {
            AmsiCloseSession(self.ctx, self.session);
            AmsiUninitialize(self.ctx);
        }
    }
}

//! Linux (e outros sistemas com `/proc`): `/proc/<pid>/status` e `/proc/<pid>/stat`.

use std::fs;

use super::ProcessMemory;

/// O processo principal. Guarda o instante de início (`starttime` do `stat`) para não
/// confundir o jogo com outro processo que receba o mesmo pid depois que ele sair.
#[derive(Debug)]
pub(super) struct Root {
    pid: u32,
    start_ticks: u64,
}

impl Root {
    pub(super) fn open(pid: u32) -> Option<Self> {
        let stat = read_stat(pid)?;
        Some(Self {
            pid,
            start_ticks: stat.start_ticks,
        })
    }

    /// `None` quando o processo saiu (ou virou zumbi, ou o pid é de outro processo).
    pub(super) fn read_root(&self) -> Option<ProcessMemory> {
        let stat = read_stat(self.pid)?;
        if stat.start_ticks != self.start_ticks || stat.state == 'Z' || stat.state == 'X' {
            return None;
        }
        read_memory(self.pid)
    }

    /// A memória de um filho; `None` se ele saiu ou começou antes do processo principal.
    pub(super) fn read_child(&self, pid: u32) -> Option<ProcessMemory> {
        let stat = read_stat(pid)?;
        if stat.start_ticks < self.start_ticks {
            return None;
        }
        read_memory(pid)
    }
}

/// A memória de um processo pelo `status`.
fn read_memory(pid: u32) -> Option<ProcessMemory> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    parse_status(&text)
}

/// Os pares `(pid, ppid)` de todos os processos.
pub(super) fn process_list() -> Option<Vec<(u32, u32)>> {
    let entries = fs::read_dir("/proc").ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter_map(|pid| Some((pid, read_stat(pid)?.ppid)))
            .collect(),
    )
}

/// Os campos do `stat` que interessam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stat {
    state: char,
    ppid: u32,
    start_ticks: u64,
}

fn read_stat(pid: u32) -> Option<Stat> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat(&text)
}

/// `pid (comm) state ppid … starttime …`. O `comm` pode ter espaços e parênteses: os campos
/// começam depois do último `)`.
fn parse_stat(text: &str) -> Option<Stat> {
    let rest = &text[text.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // Depois do `)`: o índice 0 é o campo 3 (state); o campo N fica no índice N - 3.
    Some(Stat {
        state: fields.first()?.chars().next()?,
        ppid: fields.get(1)?.parse().ok()?,
        start_ticks: fields.get(19)?.parse().ok()?,
    })
}

/// `VmRSS` e `VmHWM` (em kB) do `status`. Sem `VmRSS` (processo zumbi ou de kernel), `None`.
fn parse_status(text: &str) -> Option<ProcessMemory> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .map(|kb| kb.saturating_mul(1024))
    };
    Some(ProcessMemory {
        rss: field("VmRSS:")?,
        private: None,
        peak: field("VmHWM:"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_status() {
        let text = "Name:\tjava\nState:\tS (sleeping)\nVmPeak:\t 9000000 kB\nVmHWM:\t 2048000 kB\nVmRSS:\t 1024000 kB\nThreads:\t80\n";
        assert_eq!(
            parse_status(text),
            Some(ProcessMemory {
                rss: 1_024_000 * 1024,
                private: None,
                peak: Some(2_048_000 * 1024),
            })
        );
        // Zumbi: sem VmRSS.
        assert_eq!(parse_status("Name:\tjava\nState:\tZ (zombie)\n"), None);
    }

    #[test]
    fn le_o_stat_com_parenteses_no_nome() {
        let text = "4242 (Render (thread) 1) S 4200 4242 4242 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 80 0 987654 900000000 250000 18446744073709551615";
        assert_eq!(
            parse_stat(text),
            Some(Stat {
                state: 'S',
                ppid: 4200,
                start_ticks: 987_654,
            })
        );
        assert_eq!(parse_stat("lixo"), None);
        assert_eq!(parse_stat("1 (a) S"), None);
    }
}

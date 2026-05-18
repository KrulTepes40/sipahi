//! Stack analysis — frame map, call graph, cycle detect, summary.
//!
//! Doctrine (kullanıcı talimatı):
//! - Sum-of-frames over-approximation (raporda AÇIK belirt).
//! - Indirect call (jalr rd != x0 paired-NOT, c.jalr) → FAIL/UNKNOWN.
//! - Recursion (call graph cycle) → FAIL/UNKNOWN.
//! - Sembol çözülemedi / `.stack_sizes` eksik → FAIL/UNKNOWN.
//! - max_stack_bytes UNKNOWN sentinel = 0xFFFF_FFFF.

use crate::decode::{scan_control_flow, DirectEdge, IndirectCall};
use crate::elf::{ElfStackInfo, ElfError};
use crate::UNKNOWN_SENTINEL;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub addr: u64,
    pub name: String,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail(FailReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailReason {
    IndirectCallDetected,
    RecursionDetected,
    StackSizesMissing,
    SymbolResolveFailed,
}

impl std::fmt::Display for FailReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndirectCallDetected => write!(f, "indirect call (jalr rd != x0 or c.jalr) detected"),
            Self::RecursionDetected    => write!(f, "recursion (call graph cycle) detected"),
            Self::StackSizesMissing    => write!(f, ".stack_sizes section missing"),
            Self::SymbolResolveFailed  => write!(f, "symbol resolution failed"),
        }
    }
}

#[derive(Debug)]
pub struct AnalysisReport {
    pub frames: Vec<Frame>,
    pub indirect_calls: Vec<IndirectCall>,
    pub recursion_cycle: Option<Vec<String>>,
    pub direct_call_edges: usize,
    pub sum_of_frames: u64,
    pub status: Status,
    pub max_stack_bytes: u32,
}

pub fn analyze(elf: ElfStackInfo) -> AnalysisReport {
    let frames = join_frames_with_symbols(&elf);
    let (scan_directs, indirect_calls) = scan_control_flow(elf.text_base, &elf.text_bytes);

    // Merge: scan-derived (works on linked ELF) + relocation-derived (object files).
    let mut merged_directs: Vec<(u64, u64)> = scan_directs
        .iter()
        .map(|e: &DirectEdge| (elf.text_base + e.from_off, e.target))
        .collect();
    for &(from_off, target) in &elf.direct_calls {
        merged_directs.push((elf.text_base + from_off, target));
    }

    let cycle = detect_cycle(&merged_directs, &frames);
    let direct_call_edges = merged_directs.len();
    let sum_of_frames: u64 = frames.iter().map(|f| f.size as u64).sum();

    let status = if !indirect_calls.is_empty() {
        Status::Fail(FailReason::IndirectCallDetected)
    } else if cycle.is_some() {
        Status::Fail(FailReason::RecursionDetected)
    } else if frames.is_empty() {
        Status::Fail(FailReason::StackSizesMissing)
    } else {
        Status::Pass
    };

    let max_stack_bytes = match &status {
        Status::Pass => {
            if sum_of_frames > u32::MAX as u64 {
                u32::MAX
            } else {
                sum_of_frames as u32
            }
        }
        Status::Fail(_) => UNKNOWN_SENTINEL,
    };

    AnalysisReport {
        frames,
        indirect_calls,
        recursion_cycle: cycle,
        direct_call_edges,
        sum_of_frames,
        status,
        max_stack_bytes,
    }
}

fn join_frames_with_symbols(elf: &ElfStackInfo) -> Vec<Frame> {
    let mut out = Vec::with_capacity(elf.frames.len());
    for &(addr, size) in &elf.frames {
        let name = elf
            .symbols
            .iter()
            .find(|(s_addr, _)| *s_addr == addr)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| format!("fn_0x{:x}", addr));
        out.push(Frame { addr, name, size });
    }
    out.sort_by_key(|f| f.addr);
    out
}

/// DFS over direct call graph. Cycle ⇒ recursion.
/// Function identity = function start address. Direct edges' `from_abs` →
/// enclosing function via "closest start addr ≤ offset".
fn detect_cycle(direct_edges: &[(u64, u64)], frames: &[Frame]) -> Option<Vec<String>> {
    use std::collections::HashMap;

    if frames.is_empty() {
        return None;
    }

    let fn_starts: Vec<u64> = frames.iter().map(|f| f.addr).collect();
    let addr_to_idx: HashMap<u64, usize> =
        frames.iter().enumerate().map(|(i, f)| (f.addr, i)).collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); frames.len()];
    for &(from_abs, target_addr) in direct_edges {
        let from_fn = enclosing_function(&fn_starts, from_abs);
        let target_fn = addr_to_idx.get(&target_addr).copied();
        if let (Some(f), Some(t)) = (from_fn, target_fn) {
            if !adj[f].contains(&t) {
                adj[f].push(t);
            }
        }
    }

    enum Visit { White, Gray, Black }
    let mut color: Vec<Visit> = (0..frames.len()).map(|_| Visit::White).collect();
    let mut stack_path: Vec<usize> = Vec::new();

    fn dfs(
        u: usize, adj: &[Vec<usize>], color: &mut [Visit],
        path: &mut Vec<usize>, frames: &[Frame],
    ) -> Option<Vec<String>> {
        color[u] = Visit::Gray;
        path.push(u);
        for &v in &adj[u] {
            match color[v] {
                Visit::Gray => {
                    let cycle_start = path.iter().position(|&x| x == v).unwrap_or(0);
                    let mut names: Vec<String> =
                        path[cycle_start..].iter().map(|&i| frames[i].name.clone()).collect();
                    names.push(frames[v].name.clone());
                    return Some(names);
                }
                Visit::White => {
                    if let Some(c) = dfs(v, adj, color, path, frames) {
                        return Some(c);
                    }
                }
                Visit::Black => {}
            }
        }
        path.pop();
        color[u] = Visit::Black;
        None
    }

    for i in 0..frames.len() {
        if matches!(color[i], Visit::White) {
            if let Some(c) = dfs(i, &adj, &mut color, &mut stack_path, frames) {
                return Some(c);
            }
            stack_path.clear();
        }
    }
    None
}

fn enclosing_function(fn_starts: &[u64], addr: u64) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, &start) in fn_starts.iter().enumerate() {
        if start <= addr {
            match best {
                None => best = Some(i),
                Some(b) if fn_starts[b] < start => best = Some(i),
                _ => {}
            }
        }
    }
    best
}

impl From<ElfError> for AnalysisReport {
    fn from(e: ElfError) -> Self {
        let reason = match e {
            ElfError::MissingStackSizes => FailReason::StackSizesMissing,
            ElfError::MissingSymtab     => FailReason::SymbolResolveFailed,
            _ => FailReason::SymbolResolveFailed,
        };
        AnalysisReport {
            frames: Vec::new(),
            indirect_calls: Vec::new(),
            recursion_cycle: None,
            direct_call_edges: 0,
            sum_of_frames: 0,
            status: Status::Fail(reason),
            max_stack_bytes: UNKNOWN_SENTINEL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elf::ElfStackInfo;

    fn make_info(frames: Vec<(u64, u32)>, syms: Vec<(u64, &str)>, calls: Vec<(u64, u64)>, text: Vec<u8>) -> ElfStackInfo {
        ElfStackInfo {
            frames,
            symbols: syms.into_iter().map(|(a, n)| (a, n.to_string())).collect(),
            direct_calls: calls,
            text_base: 0x8060_0000,
            text_bytes: text,
        }
    }

    #[test]
    fn empty_text_passes_with_zero_sum() {
        let info = make_info(vec![(0x80600000, 64)], vec![(0x80600000, "main")], vec![], vec![]);
        let r = analyze(info);
        assert_eq!(r.status, Status::Pass);
        assert_eq!(r.sum_of_frames, 64);
        assert_eq!(r.max_stack_bytes, 64);
    }

    #[test]
    fn bare_jalr_fails_unknown() {
        // jalr ra, t0, 0 (rs1=5, no AUIPC) → indirect
        let raw: u32 = (5 << 15) | (1 << 7) | 0x67;
        let mut text = Vec::new();
        text.extend_from_slice(&raw.to_le_bytes());
        let info = make_info(vec![(0x80600000, 32)], vec![(0x80600000, "main")], vec![], text);
        let r = analyze(info);
        assert!(matches!(r.status, Status::Fail(FailReason::IndirectCallDetected)));
        assert_eq!(r.max_stack_bytes, UNKNOWN_SENTINEL);
    }

    #[test]
    fn auipc_jalr_resolved_pair_is_not_indirect() {
        // auipc ra, 0; jalr ra, ra, 0x10 → direct call to 0x80600010
        let mut text = Vec::new();
        text.extend_from_slice(&(((0u32) << 12) | (1 << 7) | 0x17).to_le_bytes()); // auipc ra, 0
        text.extend_from_slice(&((0x10u32 << 20) | (1 << 15) | (1 << 7) | 0x67).to_le_bytes()); // jalr ra, ra, 0x10
        // frames: main at 0x80600000 (size 32), helper at 0x80600010 (size 16)
        let info = make_info(
            vec![(0x80600000, 32), (0x80600010, 16)],
            vec![(0x80600000, "main"), (0x80600010, "helper")],
            vec![],
            text,
        );
        let r = analyze(info);
        assert_eq!(r.status, Status::Pass);
        assert_eq!(r.direct_call_edges, 1);
        assert_eq!(r.sum_of_frames, 48);
    }

    #[test]
    fn recursion_detected_via_relocation_input() {
        // Relocation input form (object-file path).
        let info = make_info(
            vec![(0x80600000, 64)],
            vec![(0x80600000, "main")],
            vec![(0, 0x80600000)],
            vec![],
        );
        let r = analyze(info);
        assert!(matches!(r.status, Status::Fail(FailReason::RecursionDetected)));
        assert_eq!(r.max_stack_bytes, UNKNOWN_SENTINEL);
    }

    #[test]
    fn no_recursion_chain_a_b_passes() {
        let info = make_info(
            vec![(0x80600000, 32), (0x80600100, 16)],
            vec![(0x80600000, "main"), (0x80600100, "helper")],
            vec![(0, 0x80600100)],
            vec![],
        );
        let r = analyze(info);
        assert_eq!(r.status, Status::Pass);
        assert_eq!(r.sum_of_frames, 48);
    }

    #[test]
    fn mutual_recursion_a_b_a_detected() {
        let info = make_info(
            vec![(0x80600000, 32), (0x80600100, 32)],
            vec![(0x80600000, "a"), (0x80600100, "b")],
            vec![(0, 0x80600100), (0x100, 0x80600000)],
            vec![],
        );
        let r = analyze(info);
        assert!(matches!(r.status, Status::Fail(FailReason::RecursionDetected)));
        assert_eq!(r.max_stack_bytes, UNKNOWN_SENTINEL);
        let cyc = r.recursion_cycle.unwrap();
        assert!(cyc.len() >= 2);
    }

    #[test]
    fn unknown_symbol_uses_address_name() {
        let info = make_info(vec![(0x80601234, 16)], vec![], vec![], vec![]);
        let r = analyze(info);
        assert_eq!(r.frames[0].name, "fn_0x80601234");
    }
}

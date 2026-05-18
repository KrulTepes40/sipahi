//! Text output format — golden fixture + parser zorunlu.
//!
//! Schema (line-oriented, key:value; sntm-validate parser consumer):
//!
//!   SNTM-STACK v1.0
//!   binary: <path>
//!   arch: rv64
//!   mode: sum-of-frames (over-approximation)
//!   caveat: call-graph-aware transitive analysis post-SAFE
//!
//!   frames:
//!     0x80600000  _start                        0 byte
//!     0x80600008  task_hello::main            128 byte
//!     ...
//!
//!   direct_call_edges: N
//!   indirect_calls: M
//!   <if M > 0:>
//!     - 0x80600124 jalr32 rd=1 rs1=5
//!   recursion: <none | A -> B -> A>
//!
//!   status: PASS|FAIL
//!   reason: <if FAIL>
//!   max_stack_bytes: <decimal> (or 0xFFFFFFFF for FAIL)
//!
//! Parser hedef satırı: `max_stack_bytes: <N>` (lowercase hex 0xFFFFFFFF or
//! decimal). G2 sntm-validate stackreport.rs bu satırı arar.

use crate::analysis::{AnalysisReport, Status};
use crate::REPORT_VERSION;

pub fn render(report: &AnalysisReport, binary_path: &str) -> String {
    let mut out = String::new();
    out.push_str(REPORT_VERSION);
    out.push('\n');
    out.push_str(&format!("binary: {}\n", binary_path));
    out.push_str("arch: rv64\n");
    out.push_str("mode: sum-of-frames (over-approximation)\n");
    out.push_str("caveat: call-graph-aware transitive analysis post-SAFE\n");
    out.push('\n');
    out.push_str("frames:\n");
    for f in &report.frames {
        out.push_str(&format!(
            "  0x{:08x}  {:<32}  {} byte\n",
            f.addr, f.name, f.size
        ));
    }
    out.push('\n');
    out.push_str(&format!("direct_call_edges: {}\n", report.direct_call_edges));
    out.push_str(&format!("indirect_calls: {}\n", report.indirect_calls.len()));
    for ic in &report.indirect_calls {
        out.push_str(&format!("  - 0x{:08x} {:?}\n", ic.offset, ic.kind));
    }
    out.push_str("recursion: ");
    match &report.recursion_cycle {
        None => out.push_str("none\n"),
        Some(cycle) => {
            out.push_str(&cycle.join(" -> "));
            out.push('\n');
        }
    }
    out.push('\n');
    match &report.status {
        Status::Pass => {
            out.push_str("status: PASS\n");
        }
        Status::Fail(r) => {
            out.push_str("status: FAIL\n");
            out.push_str(&format!("reason: {}\n", r));
        }
    }
    if report.max_stack_bytes == crate::UNKNOWN_SENTINEL {
        out.push_str("max_stack_bytes: 0xFFFFFFFF\n");
    } else {
        out.push_str(&format!("max_stack_bytes: {}\n", report.max_stack_bytes));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AnalysisReport, FailReason, Frame, Status};
    use crate::UNKNOWN_SENTINEL;

    fn dummy_pass() -> AnalysisReport {
        AnalysisReport {
            frames: vec![Frame { addr: 0x80600000, name: "main".into(), size: 128 }],
            indirect_calls: Vec::new(),
            recursion_cycle: None,
            direct_call_edges: 0,
            sum_of_frames: 128,
            status: Status::Pass,
            max_stack_bytes: 128,
        }
    }

    #[test]
    fn render_pass_contains_required_fields() {
        let out = render(&dummy_pass(), "/tmp/task.elf");
        assert!(out.starts_with("SNTM-STACK v1.0\n"));
        assert!(out.contains("status: PASS"));
        assert!(out.contains("max_stack_bytes: 128"));
        assert!(out.contains("over-approximation"));
        assert!(out.contains("0x80600000"));
        assert!(out.contains("main"));
    }

    #[test]
    fn render_fail_uses_unknown_sentinel() {
        let mut r = dummy_pass();
        r.status = Status::Fail(FailReason::IndirectCallDetected);
        r.max_stack_bytes = UNKNOWN_SENTINEL;
        let out = render(&r, "/tmp/x");
        assert!(out.contains("status: FAIL"));
        assert!(out.contains("reason: indirect call"));
        assert!(out.contains("max_stack_bytes: 0xFFFFFFFF"));
    }
}

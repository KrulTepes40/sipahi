//! sntm-stack rapor parser — SAFE-4 (sprint-u33) Plan B (Section 8 CR-2/CR-3).
//!
//! Format: `tools/sntm-stack/src/report.rs` render edilen text. Kontrat:
//!   - `SNTM-STACK v1.0`  (version banner)
//!   - `status: PASS|FAIL`
//!   - `max_stack_bytes: <decimal>` (PASS) veya `0xFFFFFFFF` (FAIL = UNKNOWN sentinel)
//!   - `reason: <msg>` (FAIL only)
//!
//! Parser bu kontratı uygular:
//!   - status: FAIL → Err (reason satırı ile)
//!   - max_stack_bytes: 0xFFFFFFFF → Err (UNKNOWN sentinel)
//!   - status: PASS + valid decimal → Ok(u32)
//!
//! G0.5 golden fixture `tools/sntm-stack/tests/fixtures/task_hello.stack.golden.txt`
//! ile validate edilir (Section 9.2 T3 doctrine).

pub const UNKNOWN_SENTINEL: u32 = 0xFFFF_FFFF;
pub const EXPECTED_BANNER_PREFIX: &str = "SNTM-STACK v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReport {
    pub status_pass: bool,
    pub max_stack_bytes: u32,
    pub reason: Option<String>,
}

/// Parse sntm-stack rapor metnini. PASS satırı ve max_stack_bytes decimal
/// gerekli. FAIL → Err (reason ile). Banner satırı yoksa Err (format drift).
pub fn parse(report: &str) -> Result<ParsedReport, String> {
    let mut saw_banner = false;
    let mut status_pass: Option<bool> = None;
    let mut max_stack: Option<u32> = None;
    let mut reason: Option<String> = None;

    for line in report.lines() {
        let line = line.trim_end();
        if line.starts_with(EXPECTED_BANNER_PREFIX) {
            saw_banner = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("status:") {
            match rest.trim() {
                "PASS" => status_pass = Some(true),
                "FAIL" => status_pass = Some(false),
                other => return Err(format!("invalid status value: {:?}", other)),
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("max_stack_bytes:") {
            let v = rest.trim();
            if v == "0xFFFFFFFF" || v == "0xffffffff" {
                max_stack = Some(UNKNOWN_SENTINEL);
            } else if let Some(hex) = v.strip_prefix("0x") {
                let n = u32::from_str_radix(hex, 16)
                    .map_err(|e| format!("invalid hex max_stack_bytes: {}: {}", v, e))?;
                max_stack = Some(n);
            } else {
                let n: u32 = v
                    .parse()
                    .map_err(|e| format!("invalid decimal max_stack_bytes: {}: {}", v, e))?;
                max_stack = Some(n);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("reason:") {
            reason = Some(rest.trim().to_string());
        }
    }

    if !saw_banner {
        return Err(format!(
            "report missing banner '{}...' — format drift?",
            EXPECTED_BANNER_PREFIX
        ));
    }
    let status_pass = status_pass.ok_or_else(|| "report missing status: line".to_string())?;
    let max_stack = max_stack.ok_or_else(|| "report missing max_stack_bytes: line".to_string())?;

    Ok(ParsedReport {
        status_pass,
        max_stack_bytes: max_stack,
        reason,
    })
}

/// Convenience: parse + check status. UNKNOWN sentinel veya FAIL → Err.
pub fn parse_max_stack_bytes(report: &str) -> Result<u32, String> {
    let p = parse(report)?;
    if !p.status_pass {
        return Err(format!(
            "stack analysis FAIL: {}",
            p.reason.unwrap_or_else(|| "unspecified".to_string())
        ));
    }
    if p.max_stack_bytes == UNKNOWN_SENTINEL {
        return Err("stack analysis returned UNKNOWN sentinel (0xFFFFFFFF)".to_string());
    }
    Ok(p.max_stack_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS_SAMPLE: &str = "\
SNTM-STACK v1.0
binary: x.elf
arch: rv64
mode: sum-of-frames (over-approximation)
caveat: call-graph-aware transitive analysis post-SAFE

frames:
  0x80600000  _start    0 byte
  0x80600008  main    128 byte

direct_call_edges: 0
indirect_calls: 0
recursion: none

status: PASS
max_stack_bytes: 128
";

    const FAIL_SAMPLE: &str = "\
SNTM-STACK v1.0
binary: x.elf

status: FAIL
reason: indirect call (jalr rd != x0 or c.jalr) detected
max_stack_bytes: 0xFFFFFFFF
";

    #[test]
    fn parse_pass_returns_decimal() {
        let r = parse_max_stack_bytes(PASS_SAMPLE).unwrap();
        assert_eq!(r, 128);
    }

    #[test]
    fn parse_fail_returns_err_with_reason() {
        let err = parse_max_stack_bytes(FAIL_SAMPLE).unwrap_err();
        assert!(err.contains("indirect call"));
    }

    #[test]
    fn parse_unknown_sentinel_pass_status_still_errs() {
        // status: PASS but max_stack_bytes is sentinel — defensive
        let report = "\
SNTM-STACK v1.0
status: PASS
max_stack_bytes: 0xFFFFFFFF
";
        let err = parse_max_stack_bytes(report).unwrap_err();
        assert!(err.contains("UNKNOWN sentinel"));
    }

    #[test]
    fn parse_missing_banner_fails() {
        let report = "status: PASS\nmax_stack_bytes: 32\n";
        let err = parse_max_stack_bytes(report).unwrap_err();
        assert!(err.contains("banner"));
    }

    #[test]
    fn parse_missing_status_fails() {
        let report = "SNTM-STACK v1.0\nmax_stack_bytes: 32\n";
        let err = parse_max_stack_bytes(report).unwrap_err();
        assert!(err.contains("status"));
    }

    #[test]
    fn parse_missing_max_stack_fails() {
        let report = "SNTM-STACK v1.0\nstatus: PASS\n";
        let err = parse_max_stack_bytes(report).unwrap_err();
        assert!(err.contains("max_stack_bytes"));
    }

    #[test]
    fn parse_garbage_max_stack_fails() {
        let report = "SNTM-STACK v1.0\nstatus: PASS\nmax_stack_bytes: notanumber\n";
        let err = parse_max_stack_bytes(report).unwrap_err();
        assert!(err.contains("invalid"));
    }
}

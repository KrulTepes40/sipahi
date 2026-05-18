//! sntm-stack rapor parser — SAFE-4 (sprint-u33) Plan B (Section 8 CR-2/CR-3).
//!
//! DUPLICATE of `tools/sntm-validate/src/stackreport.rs` — Section 8 FIX-G
//! (shared `sntm-manifest` lib crate) bu sprintte deferred; iki tool aynı
//! parser logic'i kendi içinde tutar. Bilinçli duplicate (CR-7 carry-forward).
//!
//! Format kontratı: see `tools/sntm-stack/src/report.rs` render().

pub const UNKNOWN_SENTINEL: u32 = 0xFFFF_FFFF;
pub const EXPECTED_BANNER_PREFIX: &str = "SNTM-STACK v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReport {
    pub status_pass: bool,
    pub max_stack_bytes: u32,
    pub reason: Option<String>,
}

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

/// SAFE-4 CR-4 doctrine: cert tarafı için katı kural — FAIL ya da sentinel
/// → UNKNOWN_SENTINEL yaz (cert manifest stack_size'a ASLA fallback'lamaz).
/// PASS + valid value → parsed.
pub fn parse_max_stack_or_unknown(report: &str) -> u32 {
    match parse(report) {
        Ok(p) if p.status_pass && p.max_stack_bytes != UNKNOWN_SENTINEL => p.max_stack_bytes,
        _ => UNKNOWN_SENTINEL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pass_returns_value() {
        let r = "SNTM-STACK v1.0\nstatus: PASS\nmax_stack_bytes: 128\n";
        assert_eq!(parse_max_stack_or_unknown(r), 128);
    }

    #[test]
    fn parse_fail_returns_sentinel() {
        let r = "SNTM-STACK v1.0\nstatus: FAIL\nreason: indirect\nmax_stack_bytes: 0xFFFFFFFF\n";
        assert_eq!(parse_max_stack_or_unknown(r), UNKNOWN_SENTINEL);
    }

    #[test]
    fn parse_malformed_returns_sentinel() {
        // No banner → cert tarafı conservatively UNKNOWN.
        let r = "status: PASS\nmax_stack_bytes: 128\n";
        assert_eq!(parse_max_stack_or_unknown(r), UNKNOWN_SENTINEL);
    }
}

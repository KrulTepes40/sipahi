//! 12 yasak detection (SNTM design §17.2) + demo-waiver drift guard.
//!
//! Pipeline (U-30 + U-30.1): syn AST üzerinden tek geçişte cfg-aware traversal.
//! Manifest demo_feature_waivers cfg-gated bloklar SKIP edilir, extern "C" ABI
//! annotation vs foreign block ayrılır. U-30.1 değişiklikleri:
//!   - Rule 2 (extern crate alloc): string scan → Item::ExternCrate AST node
//!   - Rule 3 (asm!): string scan → Macro::path cfg-aware
//!   - Rule 10 (core::sync::atomic): string scan → Item::Use + ExprPath
//!   - UnsafeVisitor + MmioCastVisitor birleştirildi → ForbiddenVisitor
//!   - Rule 12 (drift guard): demo_feature_waivers her item Cargo.toml [features]'de
//!     tanımlı + default listesinde değil — sntm-validate ile defense-in-depth.

use crate::TaskEntry;
use std::path::Path;
use syn::visit::Visit;

const TOTAL_RULES: usize = 12;

#[derive(Debug, Default)]
pub struct LintResult {
    pub rules_passed: Vec<&'static str>,
    pub rules_failed: Vec<(String, String)>,
    pub waivers_logged: Vec<String>,
}

pub fn lint_task(task: &TaskEntry, task_dir: &Path) -> Result<String, String> {
    // SAFE-1 DAL × trust_tier policy matrix (drift guard — sntm-validate ile aynı).
    let dal = parse_dal(&task.dal_level)?;
    let tier = task.trust_tier.as_str();
    if tier != "safe" && tier != "trusted_unsafe" {
        return Err(format!(
            "invalid trust_tier '{}' (must be 'safe' or 'trusted_unsafe')",
            tier
        ));
    }
    if tier == "trusted_unsafe" && (dal == DalLevel::A || dal == DalLevel::B) {
        return Err(format!(
            "DAL-{:?} forbids trust_tier='trusted_unsafe' (DO-178C cert doctrine)",
            dal
        ));
    }

    let main_rs = task_dir.join("src").join("main.rs");
    let content = std::fs::read_to_string(&main_rs)
        .map_err(|e| format!("cannot read {}: {}", main_rs.display(), e))?;
    let ast: syn::File = syn::parse_str(&content)
        .map_err(|e| format!("syn parse error: {}", e))?;

    let demo_waivers: Vec<String> = task.demo_feature_waivers.clone();
    let mut result = LintResult::default();

    // ── Tek geçişli AST visitor (Rule 1 + 2 + 3 + 10 + 11) ──
    // trusted_unsafe tier: unsafe/asm yasakları bypass (waiver_reason ile logged).
    let mut fv = ForbiddenVisitor::new(&demo_waivers);
    fv.visit_file(&ast);

    // (1) unsafe blocks
    if tier == "trusted_unsafe" {
        result.waivers_logged.push(format!(
            "[1] unsafe blocks WAIVED (trust_tier=trusted_unsafe, reason: {})",
            task.waiver_reason
        ));
        result.rules_passed.push("unsafe blocks (waived)");
    } else if fv.unsafe_violations > 0 {
        result.rules_failed.push((
            "unsafe blocks".into(),
            format!("{} unsafe block (cfg-gated demo waiver dışı)", fv.unsafe_violations),
        ));
    } else {
        if fv.unsafe_waived > 0 {
            result.waivers_logged.push(format!(
                "[1] unsafe blocks: {} cfg-gated block waived (demo_feature_waivers)",
                fv.unsafe_waived
            ));
        }
        result.rules_passed.push("unsafe blocks");
    }

    // (2) extern crate alloc — cfg-aware
    if fv.alloc_violations > 0 {
        result.rules_failed.push((
            "extern crate alloc".into(),
            format!("{} alloc crate referansı (cfg-gated waiver dışı)", fv.alloc_violations),
        ));
    } else {
        if fv.alloc_waived > 0 {
            result.waivers_logged.push(format!(
                "[2] extern crate alloc: {} cfg-gated waived", fv.alloc_waived
            ));
        }
        result.rules_passed.push("extern crate alloc");
    }

    // (3) asm! / core::arch::asm — cfg-aware
    if tier == "trusted_unsafe" {
        result.waivers_logged.push("[3] asm! WAIVED (trusted_unsafe)".into());
        result.rules_passed.push("core::arch::asm! (waived)");
    } else if fv.asm_violations > 0 {
        result.rules_failed.push((
            "core::arch::asm!".into(),
            format!("{} asm macro / core::arch::asm path (cfg-gated waiver dışı)", fv.asm_violations),
        ));
    } else {
        if fv.asm_waived > 0 {
            result.waivers_logged.push(format!(
                "[3] asm!: {} cfg-gated waived", fv.asm_waived
            ));
        }
        result.rules_passed.push("core::arch::asm!");
    }

    // (4) extern "C" foreign block — ABI annotation izinli (cfg-blind: hiç olmamalı)
    let ffi_count = count_foreign_mods(&ast);
    if ffi_count > 0 {
        result.rules_failed.push((
            "extern \"C\" foreign block".into(),
            format!("{} foreign mod (extern block) bulundu", ffi_count),
        ));
    } else {
        result.rules_passed.push("extern \"C\" foreign block");
    }

    // (5) direct recursion (SAFE-4 indirect call graph defer)
    let recursive = find_direct_recursion(&ast);
    if !recursive.is_empty() {
        result.rules_failed.push((
            "direct recursion".into(),
            format!("direct recursion: {:?}", recursive),
        ));
    } else {
        result.rules_passed.push("direct recursion");
    }

    // (6) dyn Trait / fn ptr
    let (dyn_count, fnptr_count) = count_dyn_fnptr(&ast);
    if dyn_count > 0 || fnptr_count > 0 {
        result.rules_failed.push((
            "dyn Trait / fn ptr".into(),
            format!("dyn={}, fnptr={}", dyn_count, fnptr_count),
        ));
    } else {
        result.rules_passed.push("dyn Trait / fn ptr");
    }

    // (7) panic = abort (workspace Cargo.toml profile.release)
    match check_panic_abort(task_dir) {
        Ok(()) => result.rules_passed.push("panic = abort"),
        Err(e) => result.rules_failed.push(("panic = unwind".into(), e)),
    }

    // (8) Global runtime init (.init_array)
    if content.contains(r#"link_section = ".init_array""#) {
        result.rules_failed.push((
            "init_array".into(),
            "link_section = \".init_array\" attribute bulundu".into(),
        ));
    } else {
        result.rules_passed.push(".init_array");
    }

    // (9) F/D float — target gate (RV64IMAC bypass)
    result.rules_passed.push("F/D float (target gate)");

    // (10) core::sync::atomic — cfg-aware AST
    if fv.atomic_violations > 0 {
        result.rules_failed.push((
            "core::sync::atomic".into(),
            format!("{} core::sync::atomic referansı (cfg-gated waiver dışı)", fv.atomic_violations),
        ));
    } else {
        if fv.atomic_waived > 0 {
            result.waivers_logged.push(format!(
                "[10] core::sync::atomic: {} cfg-gated waived", fv.atomic_waived
            ));
        }
        result.rules_passed.push("core::sync::atomic");
    }

    // (11) MMIO direct cast (cfg-aware)
    if fv.mmio_violations > 0 {
        result.rules_failed.push((
            "MMIO direct cast".into(),
            format!("{} integer literal → raw pointer cast (cfg-gated waiver dışı)", fv.mmio_violations),
        ));
    } else {
        if fv.mmio_waived > 0 {
            result.waivers_logged.push(format!(
                "[11] MMIO direct cast: {} cfg-gated cast waived", fv.mmio_waived
            ));
        }
        result.rules_passed.push("MMIO direct cast");
    }

    // (12) demo_feature_waivers drift guard — Cargo.toml [features] cross-check.
    // U-30.1 (defense-in-depth): sntm-validate aynı check'i yapar, task-lint
    // task crate Cargo.toml'unu ayrıca okur (drift guard).
    match check_demo_waivers_cargo(task_dir, &demo_waivers) {
        Ok(()) => result.rules_passed.push("demo_feature_waivers drift"),
        Err(e) => result.rules_failed.push(("demo_feature_waivers drift".into(), e)),
    }

    if !result.rules_failed.is_empty() {
        let msgs: Vec<String> = result.rules_failed.iter()
            .map(|(r, e)| format!("  [FAIL] {}: {}", r, e))
            .collect();
        return Err(format!(
            "{} rule(s) failed:\n{}",
            result.rules_failed.len(),
            msgs.join("\n")
        ));
    }

    let mut report = format!("PASS: {} (trust_tier={}", task.name, tier);
    if !demo_waivers.is_empty() {
        report.push_str(&format!(", demo_feature_waivers={:?}", demo_waivers));
    }
    report.push_str(&format!(", {}/{} rules)", result.rules_passed.len(), TOTAL_RULES));
    for w in &result.waivers_logged {
        report.push_str(&format!("\n  [waiver] {}", w));
    }
    Ok(report)
}

// ─── DAL enum ───────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum DalLevel { A, B, C, D }

fn parse_dal(s: &str) -> Result<DalLevel, String> {
    match s {
        "A" => Ok(DalLevel::A),
        "B" => Ok(DalLevel::B),
        "C" => Ok(DalLevel::C),
        "D" => Ok(DalLevel::D),
        _   => Err(format!("invalid dal_level: {}", s)),
    }
}

// ─── ForbiddenVisitor — single cfg-aware AST pass ──────────────
//
// Tracks: unsafe blocks, extern crate alloc, asm! macros, core::sync::atomic
// paths, integer-literal → raw pointer casts. Cfg-gated demo_feature_waivers
// matchli scope'lar skipped'a düşer. UnsafeVisitor + MmioCastVisitor U-30
// duplicate'i U-30.1'de birleştirildi.

struct ForbiddenVisitor {
    demo_waivers: Vec<String>,
    in_waived_scope: u32,
    unsafe_violations: usize,
    unsafe_waived: usize,
    alloc_violations: usize,
    alloc_waived: usize,
    asm_violations: usize,
    asm_waived: usize,
    atomic_violations: usize,
    atomic_waived: usize,
    mmio_violations: usize,
    mmio_waived: usize,
}

impl ForbiddenVisitor {
    fn new(waivers: &[String]) -> Self {
        Self {
            demo_waivers: waivers.to_vec(),
            in_waived_scope: 0,
            unsafe_violations: 0,
            unsafe_waived: 0,
            alloc_violations: 0,
            alloc_waived: 0,
            asm_violations: 0,
            asm_waived: 0,
            atomic_violations: 0,
            atomic_waived: 0,
            mmio_violations: 0,
            mmio_waived: 0,
        }
    }

    fn is_waived_cfg(&self, attrs: &[syn::Attribute]) -> bool {
        for attr in attrs {
            if attr.path().is_ident("cfg") {
                let s = meta_to_string(&attr.meta);
                for waiver in &self.demo_waivers {
                    let pattern = format!("feature = \"{}\"", waiver);
                    if s.contains(&pattern) {
                        return true;
                    }
                }
            }
        }
        false
    }

    #[inline]
    fn bump(&mut self, kind: Kind) {
        let waived = self.in_waived_scope > 0;
        match (kind, waived) {
            (Kind::Unsafe, true)  => self.unsafe_waived += 1,
            (Kind::Unsafe, false) => self.unsafe_violations += 1,
            (Kind::Alloc, true)   => self.alloc_waived += 1,
            (Kind::Alloc, false)  => self.alloc_violations += 1,
            (Kind::Asm, true)     => self.asm_waived += 1,
            (Kind::Asm, false)    => self.asm_violations += 1,
            (Kind::Atomic, true)  => self.atomic_waived += 1,
            (Kind::Atomic, false) => self.atomic_violations += 1,
            (Kind::Mmio, true)    => self.mmio_waived += 1,
            (Kind::Mmio, false)   => self.mmio_violations += 1,
        }
    }
}

#[derive(Clone, Copy)]
enum Kind { Unsafe, Alloc, Asm, Atomic, Mmio }

impl<'ast> Visit<'ast> for ForbiddenVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        // Cfg scope enter: cfg attribute → in_waived_scope artar.
        let attrs: Vec<syn::Attribute> = match item {
            syn::Item::Fn(f)          => f.attrs.clone(),
            syn::Item::Mod(m)         => m.attrs.clone(),
            syn::Item::Impl(i)        => i.attrs.clone(),
            syn::Item::ExternCrate(e) => e.attrs.clone(),
            syn::Item::Use(u)         => u.attrs.clone(),
            syn::Item::Macro(m)       => m.attrs.clone(),
            _ => vec![],
        };
        let waived = self.is_waived_cfg(&attrs);
        if waived { self.in_waived_scope += 1; }

        // Item-level yasak detection (cfg scope INCLUDE — yani current item).
        // Rule 2: extern crate alloc
        if let syn::Item::ExternCrate(ec) = item {
            if ec.ident == "alloc" {
                self.bump(Kind::Alloc);
            }
        }
        // Rule 10: use core::sync::atomic::...
        if let syn::Item::Use(u) = item {
            let mut found = false;
            collect_use_paths(&u.tree, &mut Vec::new(), &mut |path| {
                if path_starts_with_strs(path, &["core", "sync", "atomic"]) {
                    found = true;
                }
            });
            if found {
                self.bump(Kind::Atomic);
            }
        }
        // Rule 3: top-level macro item (e.g. core::arch::global_asm! at item level)
        if let syn::Item::Macro(m) = item {
            if is_asm_macro(&m.mac.path) {
                self.bump(Kind::Asm);
            }
        }

        syn::visit::visit_item(self, item);
        if waived { self.in_waived_scope -= 1; }
    }

    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        let attrs: Vec<syn::Attribute> = match expr {
            syn::Expr::Block(b)  => b.attrs.clone(),
            syn::Expr::Unsafe(u) => u.attrs.clone(),
            _ => vec![],
        };
        let waived = self.is_waived_cfg(&attrs);
        if waived { self.in_waived_scope += 1; }
        syn::visit::visit_expr(self, expr);
        if waived { self.in_waived_scope -= 1; }
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.bump(Kind::Unsafe);
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if is_asm_macro(&node.mac.path) {
            self.bump(Kind::Asm);
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if is_asm_macro(&node.mac.path) {
            self.bump(Kind::Asm);
        }
        syn::visit::visit_stmt_macro(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if path_starts_with(&node.path, &["core", "sync", "atomic"])
            || path_starts_with(&node.path, &["core", "arch", "asm"])
        {
            if path_starts_with(&node.path, &["core", "arch", "asm"]) {
                self.bump(Kind::Asm);
            } else {
                self.bump(Kind::Atomic);
            }
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if path_starts_with(&node.path, &["core", "sync", "atomic"]) {
            self.bump(Kind::Atomic);
        }
        syn::visit::visit_type_path(self, node);
    }

    fn visit_expr_cast(&mut self, node: &'ast syn::ExprCast) {
        let is_int_literal = matches!(&*node.expr, syn::Expr::Lit(l)
            if matches!(l.lit, syn::Lit::Int(_)));
        let is_raw_ptr = matches!(&*node.ty, syn::Type::Ptr(_));
        if is_int_literal && is_raw_ptr {
            self.bump(Kind::Mmio);
        }
        syn::visit::visit_expr_cast(self, node);
    }
}

// ─── Path helpers ──────────────────────────────────────────────

fn is_asm_macro(path: &syn::Path) -> bool {
    // Match: `asm`, `core::arch::asm`, `global_asm`, `core::arch::global_asm`.
    let segs: Vec<String> = path.segments.iter()
        .map(|s| s.ident.to_string()).collect();
    let segs_ref: Vec<&str> = segs.iter().map(String::as_str).collect();
    match segs_ref.as_slice() {
        [last] if *last == "asm" || *last == "global_asm" => true,
        [.., "arch", last] if *last == "asm" || *last == "global_asm" => true,
        _ => false,
    }
}

fn path_starts_with(path: &syn::Path, prefix: &[&str]) -> bool {
    if path.segments.len() < prefix.len() {
        return false;
    }
    path.segments.iter().take(prefix.len())
        .zip(prefix.iter())
        .all(|(s, p)| s.ident == *p)
}

fn collect_use_paths<F: FnMut(&[String])>(
    tree: &syn::UseTree,
    stack: &mut Vec<String>,
    sink: &mut F,
) {
    match tree {
        syn::UseTree::Path(p) => {
            stack.push(p.ident.to_string());
            collect_use_paths(&p.tree, stack, sink);
            stack.pop();
        }
        syn::UseTree::Name(n) => {
            stack.push(n.ident.to_string());
            sink(stack);
            stack.pop();
        }
        syn::UseTree::Rename(r) => {
            stack.push(r.ident.to_string());
            sink(stack);
            stack.pop();
        }
        syn::UseTree::Glob(_) => {
            sink(stack);
        }
        syn::UseTree::Group(g) => {
            for inner in &g.items {
                collect_use_paths(inner, stack, sink);
            }
        }
    }
}

fn path_starts_with_strs(path: &[String], prefix: &[&str]) -> bool {
    if path.len() < prefix.len() {
        return false;
    }
    path.iter().take(prefix.len()).zip(prefix.iter()).all(|(s, p)| s == p)
}

fn meta_to_string(meta: &syn::Meta) -> String {
    match meta {
        syn::Meta::List(list) => format!("{}", list.tokens),
        syn::Meta::NameValue(nv) => match &nv.value {
            syn::Expr::Lit(l) => match &l.lit {
                syn::Lit::Str(s) => format!("= \"{}\"", s.value()),
                _ => String::new(),
            },
            _ => String::new(),
        },
        syn::Meta::Path(p) => p.segments.iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>().join("::"),
    }
}

// ─── Rule 4: extern "C" foreign block ──────────────────────────

fn count_foreign_mods(ast: &syn::File) -> usize {
    let mut visitor = ForeignVisitor { count: 0 };
    visitor.visit_file(ast);
    visitor.count
}

struct ForeignVisitor { count: usize }
impl<'ast> Visit<'ast> for ForeignVisitor {
    fn visit_item_foreign_mod(&mut self, _node: &'ast syn::ItemForeignMod) {
        self.count += 1;
    }
}

// ─── Rule 5: direct recursion ──────────────────────────────────

fn find_direct_recursion(ast: &syn::File) -> Vec<String> {
    let mut recursive = Vec::new();
    for item in &ast.items {
        if let syn::Item::Fn(f) = item {
            let fn_name = f.sig.ident.to_string();
            let mut visitor = CallVisitor { target: fn_name.clone(), found: false };
            visitor.visit_block(&f.block);
            if visitor.found {
                recursive.push(fn_name);
            }
        }
    }
    recursive
}

struct CallVisitor { target: String, found: bool }
impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            if let Some(last) = p.path.segments.last() {
                if last.ident == self.target {
                    self.found = true;
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

// ─── Rule 6: dyn Trait / fn ptr ────────────────────────────────

fn count_dyn_fnptr(ast: &syn::File) -> (usize, usize) {
    let mut visitor = DynFnVisitor { dyn_count: 0, fnptr_count: 0 };
    visitor.visit_file(ast);
    (visitor.dyn_count, visitor.fnptr_count)
}

struct DynFnVisitor { dyn_count: usize, fnptr_count: usize }
impl<'ast> Visit<'ast> for DynFnVisitor {
    fn visit_type_trait_object(&mut self, _node: &'ast syn::TypeTraitObject) {
        self.dyn_count += 1;
    }
    fn visit_type_bare_fn(&mut self, _node: &'ast syn::TypeBareFn) {
        self.fnptr_count += 1;
    }
}

// ─── Rule 7: panic = abort (workspace Cargo.toml) ──────────────

fn check_panic_abort(task_dir: &Path) -> Result<(), String> {
    let workspace_root = task_dir.parent()
        .and_then(|p| p.parent())
        .ok_or("workspace root bulunamadı")?;
    let workspace_cargo = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&workspace_cargo)
        .map_err(|e| format!("workspace Cargo.toml okunamadı: {}", e))?;
    if !content.contains("panic = \"abort\"") {
        return Err("workspace Cargo.toml [profile.release] panic = \"abort\" eksik".into());
    }
    Ok(())
}

// ─── Rule 12: demo waivers Cargo.toml drift guard ──────────────
//
// Her demo_feature_waivers item için:
//   1. task_dir/Cargo.toml [features] tablosunda tanımlı mı? (orphan check)
//   2. [features.default] dizisinde mi? (default-ON yasak — drift waiver)

fn check_demo_waivers_cargo(task_dir: &Path, waivers: &[String]) -> Result<(), String> {
    if waivers.is_empty() {
        return Ok(());
    }
    let cargo = task_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo)
        .map_err(|e| format!("cannot read {}: {}", cargo.display(), e))?;
    let parsed: toml::Value = toml::from_str(&content)
        .map_err(|e| format!("task Cargo.toml parse error: {}", e))?;

    let features = match parsed.get("features") {
        Some(toml::Value::Table(t)) => t,
        Some(_) => return Err("[features] must be a table".into()),
        None => return Err(format!(
            "demo_feature_waivers={:?} but [features] table missing in Cargo.toml",
            waivers
        )),
    };

    let default_list: Vec<String> = features
        .get("default")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect())
        .unwrap_or_default();

    let mut errs = Vec::new();
    for w in waivers {
        if !features.contains_key(w) {
            errs.push(format!("waiver '{}' orphan (not in [features])", w));
            continue;
        }
        if default_list.iter().any(|d| d == w) {
            errs.push(format!(
                "waiver '{}' is in [features.default] (must be default-OFF; drift)",
                w
            ));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
}

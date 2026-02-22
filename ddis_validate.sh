#!/usr/bin/env bash
# ddis_validate.sh — Validates DDIS modular spec consistency
#
# Implements §0.13.11 (Consistency Validation) and §0.13.12 (Cascade Protocol)
# of the DDIS Standard v1.0. Runs nine mechanical checks against manifest.yaml
# and the referenced files.
#
# Usage:
#   ddis_validate.sh [OPTIONS]
#
# Options:
#   -m, --manifest PATH       Path to manifest.yaml (default: ./manifest.yaml)
#   -c, --check CHECK_NUM     Run only specific check(s), comma-separated (e.g., 1,3,7)
#   --check-cascade INV_ID    Run cascade analysis for a changed invariant
#   --with-beads              Create/reopen br issues for cascade results
#   -q, --quiet               Only show errors, suppress info and warnings
#   -v, --verbose             Show per-check detail even for passing checks
#   --json                    Output results as JSON
#   -h, --help                Show this help
#
# Exit codes:
#   0  All checks pass
#   1  One or more checks fail
#   2  Manifest parse error or configuration problem

set -euo pipefail

# --- Defaults ---
MANIFEST="./manifest.yaml"
CHECKS=""
CASCADE_INV=""
WITH_BEADS=false
QUIET=false
VERBOSE=false
JSON_OUTPUT=false

# --- Argument parsing ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        -m|--manifest)       MANIFEST="$2"; shift 2 ;;
        -c|--check)          CHECKS="$2"; shift 2 ;;
        --check-cascade)     CASCADE_INV="$2"; shift 2 ;;
        --with-beads)        WITH_BEADS=true; shift ;;
        -q|--quiet)          QUIET=true; shift ;;
        -v|--verbose)        VERBOSE=true; shift ;;
        --json)              JSON_OUTPUT=true; shift ;;
        -h|--help)           sed -n '2,/^$/{ s/^# //; s/^#//; p; }' "$0"; exit 0 ;;
        -*)                  echo "[ERROR] Unknown option: $1" >&2; exit 2 ;;
        *)                   echo "[ERROR] Unexpected argument: $1" >&2; exit 2 ;;
    esac
done

[[ -f "$MANIFEST" ]] || { echo "[ERROR] Manifest not found: $MANIFEST" >&2; exit 2; }

MANIFEST_DIR="$(cd "$(dirname "$MANIFEST")" && pwd)"
MANIFEST="$(cd "$(dirname "$MANIFEST")" && pwd)/$(basename "$MANIFEST")"

python3 - "$MANIFEST" "$CHECKS" "$CASCADE_INV" "$WITH_BEADS" "$QUIET" "$VERBOSE" "$JSON_OUTPUT" <<'PYTHON_EOF'
import sys
import os
import re
import json
import yaml
import glob as globmod
from pathlib import Path

def main():
    args = sys.argv[1:]
    manifest_path = args[0]
    checks_str = args[1]
    cascade_inv = args[2]
    with_beads = args[3].lower() == "true"
    quiet = args[4].lower() == "true"
    verbose = args[5].lower() == "true"
    json_output = args[6].lower() == "true"

    # --- Color helpers (disabled for JSON output) ---
    use_color = not json_output and sys.stderr.isatty()
    def c(code):
        return f"\033[{code}m" if use_color else ""
    RED, YELLOW, GREEN, CYAN, BOLD, RESET = c("0;31"), c("0;33"), c("0;32"), c("0;36"), c("1"), c("0")

    def info(msg):
        if not quiet and not json_output:
            print(f"{CYAN}[INFO]{RESET} {msg}")
    def warn(msg):
        if not json_output:
            print(f"{YELLOW}[WARN]{RESET} {msg}", file=sys.stderr)
    def error(msg):
        if not json_output:
            print(f"{RED}[ERROR]{RESET} {msg}", file=sys.stderr)

    # --- Parse manifest ---
    try:
        with open(manifest_path) as f:
            manifest = yaml.safe_load(f)
    except (yaml.YAMLError, FileNotFoundError) as e:
        error(f"Failed to parse manifest: {e}")
        sys.exit(2)

    manifest_dir = os.path.dirname(os.path.abspath(manifest_path))
    def resolve(p):
        return Path(manifest_dir) / p if p else None

    tier_mode = manifest.get("tier_mode", "three-tier")
    budget = manifest.get("context_budget", {})
    target_lines = budget.get("target_lines", 4000)
    hard_ceiling = budget.get("hard_ceiling_lines", 5000)
    constitution = manifest.get("constitution", {})
    system_file = constitution.get("system")
    domains = constitution.get("domains", {}) or {}
    modules = manifest.get("modules", {}) or {}
    inv_registry = manifest.get("invariant_registry", {}) or {}

    # Determine which checks to run
    if checks_str:
        requested_checks = set(int(x.strip()) for x in checks_str.split(","))
    else:
        requested_checks = set(range(1, 10))

    # --- Cascade mode ---
    if cascade_inv:
        run_cascade(cascade_inv, modules, inv_registry, with_beads, quiet, json_output,
                    RED, YELLOW, GREEN, CYAN, BOLD, RESET)
        return

    # --- Validation checks ---
    results = {}
    total_errors = 0

    def run_check(num, name, func):
        nonlocal total_errors
        if num not in requested_checks:
            return
        errors = func()
        passed = len(errors) == 0
        results[num] = {"name": name, "passed": passed, "errors": errors}
        total_errors += len(errors)
        if not json_output:
            status = f"{GREEN}PASS{RESET}" if passed else f"{RED}FAIL ({len(errors)} error(s)){RESET}"
            if passed and not verbose and not quiet:
                print(f"  CHECK-{num}: {name} ... {status}")
            elif not passed:
                print(f"  CHECK-{num}: {name} ... {status}")
                for e in errors:
                    print(f"    {RED}→{RESET} {e}")
            elif verbose:
                print(f"  CHECK-{num}: {name} ... {status}")

    info(f"Validating manifest: {os.path.basename(manifest_path)} ({tier_mode} mode, {len(modules)} modules)")
    if not json_output:
        print()

    # --- CHECK-1: Invariant ownership completeness ---
    def check_1():
        errors = []
        for inv_id, inv_cfg in inv_registry.items():
            owner = inv_cfg.get("owner", "")
            maintainers = [m for m, mcfg in modules.items() if inv_id in (mcfg.get("maintains") or [])]
            if owner == "system":
                if len(maintainers) > 0:
                    errors.append(f"{inv_id}: system-owned but maintained by {maintainers}")
            else:
                if len(maintainers) == 0:
                    errors.append(f"{inv_id}: non-system owned but no module maintains it")
                elif len(maintainers) > 1:
                    errors.append(f"{inv_id}: maintained by multiple modules: {maintainers}")
        return errors
    run_check(1, "Invariant ownership completeness", check_1)

    # --- CHECK-2: Interface consistency ---
    def check_2():
        errors = []
        for mname, mcfg in modules.items():
            ifaces = mcfg.get("interfaces") or []
            if ifaces == "all":
                continue
            for inv_id in ifaces:
                # Must be maintained by some other module, or be system-owned
                inv_cfg = inv_registry.get(inv_id, {})
                is_system = inv_cfg.get("owner") == "system"
                maintained_elsewhere = any(
                    inv_id in (ocfg.get("maintains") or [])
                    for oname, ocfg in modules.items() if oname != mname
                )
                if not is_system and not maintained_elsewhere:
                    errors.append(f"{mname} interfaces {inv_id}: not maintained by any other module and not system-owned")
        return errors
    run_check(2, "Interface consistency", check_2)

    # --- CHECK-3: Adjacency symmetry ---
    def check_3():
        errors = []
        for aname, acfg in modules.items():
            a_adj = acfg.get("adjacent") or []
            if a_adj == "all":
                continue
            for bname in a_adj:
                if bname not in modules:
                    errors.append(f"{aname} lists adjacent '{bname}' which doesn't exist in manifest")
                    continue
                bcfg = modules[bname]
                b_adj = bcfg.get("adjacent") or []
                if b_adj == "all":
                    continue
                if aname not in b_adj:
                    errors.append(f"{aname} → {bname} (asymmetric: {bname} does not list {aname})")
        return errors
    run_check(3, "Adjacency symmetry", check_3)

    # --- CHECK-4: Domain membership consistency ---
    def check_4():
        errors = []
        for mname, mcfg in modules.items():
            mdomain = mcfg.get("domain", "")
            if mdomain == "cross-cutting":
                continue
            for inv_id in (mcfg.get("maintains") or []):
                inv_cfg = inv_registry.get(inv_id, {})
                inv_domain = inv_cfg.get("domain", "")
                if inv_domain != mdomain and inv_domain != "system":
                    errors.append(f"{mname} (domain={mdomain}) maintains {inv_id} (domain={inv_domain})")
        return errors
    run_check(4, "Domain membership consistency", check_4)

    # --- CHECK-5: Budget compliance ---
    def check_5():
        errors = []
        for mname, mcfg in modules.items():
            total = 0
            # System constitution
            sp = resolve(system_file)
            if sp and sp.exists():
                total += count_lines(sp)
            # Domain constitution(s)
            if tier_mode == "three-tier":
                mdomain = mcfg.get("domain", "")
                if mdomain == "cross-cutting":
                    for dcfg in domains.values():
                        df = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
                        dp = resolve(df)
                        if dp and dp.exists():
                            total += count_lines(dp)
                elif mdomain in domains:
                    dcfg = domains[mdomain]
                    df = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
                    dp = resolve(df)
                    if dp and dp.exists():
                        total += count_lines(dp)
                # Deep context
                dc = mcfg.get("deep_context")
                if dc:
                    dcp = resolve(dc)
                    if dcp and dcp.exists():
                        total += count_lines(dcp)
            # Module file
            mf = resolve(mcfg.get("file"))
            if mf and mf.exists():
                total += count_lines(mf)
            if total > hard_ceiling:
                errors.append(f"{mname}: {total} lines exceeds hard ceiling {hard_ceiling} (INV-014 violated)")
        return errors
    run_check(5, "Budget compliance", check_5)

    # --- CHECK-6: No orphan invariants ---
    def check_6():
        errors = []
        for inv_id in inv_registry:
            referenced = False
            for mname, mcfg in modules.items():
                maintains = mcfg.get("maintains") or []
                ifaces = mcfg.get("interfaces") or []
                if inv_id in maintains:
                    referenced = True
                    break
                if ifaces == "all" or inv_id in ifaces:
                    referenced = True
                    break
            if not referenced:
                errors.append(f"{inv_id}: not referenced by any module's maintains or interfaces")
        return errors
    run_check(6, "No orphan invariants", check_6)

    # --- CHECK-7: Cross-module reference isolation ---
    def check_7():
        errors = []
        module_names = set(modules.keys())
        # Build patterns: references to other modules' internal sections
        # Look for patterns like "see <module_name>'s" or "in the <module_name> module's"
        # or direct file references to other module files
        module_files_map = {}
        for mname, mcfg in modules.items():
            mf = mcfg.get("file", "")
            module_files_map[mname] = mf

        for mname, mcfg in modules.items():
            mf = resolve(mcfg.get("file"))
            if not mf or not mf.exists():
                continue
            content = mf.read_text()
            other_modules = module_names - {mname}
            for other in other_modules:
                # Check for direct references to other module's internals
                # Pattern: references like "the <other> module's <internal>" or
                # "see <other>'s" or filename references
                patterns = [
                    rf'\b{re.escape(other)}(?:\'s|_module\'s)\s+(?:internal|implementation|algorithm|function|method)',
                    rf'(?:in|see|from)\s+(?:the\s+)?{re.escape(other)}\s+module',
                    rf'modules/{re.escape(other)}\.md',
                ]
                for pat in patterns:
                    matches = re.findall(pat, content, re.IGNORECASE)
                    if matches:
                        for m in matches:
                            errors.append(f"{mname} → {other}: direct reference found: '{m}' (violates INV-012)")
        return errors
    run_check(7, "Cross-module reference isolation", check_7)

    # --- CHECK-8: Deep context correctness (three-tier only) ---
    def check_8():
        errors = []
        if tier_mode != "three-tier":
            return errors
        for mname, mcfg in modules.items():
            mdomain = mcfg.get("domain", "")
            if mdomain == "cross-cutting":
                continue
            # Find cross-domain interfaces
            ifaces = mcfg.get("interfaces") or []
            if ifaces == "all":
                # Cross-cutting effectively; skip check
                continue
            cross_domain = []
            for inv_id in ifaces:
                inv_cfg = inv_registry.get(inv_id, {})
                inv_domain = inv_cfg.get("domain", "")
                if inv_domain != mdomain and inv_domain != "system":
                    cross_domain.append(inv_id)
            has_deep = mcfg.get("deep_context") is not None
            if cross_domain and not has_deep:
                errors.append(f"{mname}: has cross-domain interfaces {cross_domain} but no deep_context")
            elif not cross_domain and has_deep:
                errors.append(f"{mname}: has deep_context but no cross-domain interfaces (unnecessary)")
        return errors
    run_check(8, "Deep context correctness", check_8)

    # --- CHECK-9: File existence ---
    def check_9():
        errors = []
        # All referenced paths must exist
        paths_to_check = []
        if system_file:
            paths_to_check.append(("constitution.system", system_file))
        for dname, dcfg in domains.items():
            df = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
            paths_to_check.append((f"constitution.domains.{dname}", df))
        for mname, mcfg in modules.items():
            paths_to_check.append((f"modules.{mname}.file", mcfg.get("file")))
            dc = mcfg.get("deep_context")
            if dc:
                paths_to_check.append((f"modules.{mname}.deep_context", dc))
        for label, p in paths_to_check:
            rp = resolve(p)
            if not rp or not rp.exists():
                errors.append(f"{label}: file not found: {p}")

        # Reverse check: module files on disk not in manifest
        modules_dir = Path(manifest_dir) / "modules"
        if modules_dir.exists():
            manifest_files = set()
            for mcfg in modules.values():
                mf = mcfg.get("file", "")
                manifest_files.add(os.path.basename(mf))
            for f in modules_dir.iterdir():
                if f.is_file() and f.suffix == ".md":
                    if f.name not in manifest_files:
                        errors.append(f"modules/{f.name}: exists on disk but not in manifest (INV-016)")
        return errors
    run_check(9, "File existence", check_9)

    # --- Output ---
    if json_output:
        output = {
            "manifest": os.path.basename(manifest_path),
            "tier_mode": tier_mode,
            "module_count": len(modules),
            "checks": {},
            "total_errors": total_errors,
            "passed": total_errors == 0,
        }
        for num, result in sorted(results.items()):
            output["checks"][f"CHECK-{num}"] = result
        print(json.dumps(output, indent=2))
    else:
        print()
        if total_errors == 0:
            print(f"{BOLD}{GREEN}All checks passed.{RESET}")
        else:
            print(f"{BOLD}{RED}{total_errors} error(s) across {sum(1 for r in results.values() if not r['passed'])} failing check(s).{RESET}")

    sys.exit(0 if total_errors == 0 else 1)


def count_lines(path):
    try:
        content = path.read_text()
        return content.count('\n') + (1 if content and not content.endswith('\n') else 0)
    except FileNotFoundError:
        return 0


def run_cascade(inv_id, modules, inv_registry, with_beads, quiet, json_output,
                RED, YELLOW, GREEN, CYAN, BOLD, RESET):
    """Cascade analysis: given a changed invariant, identify affected modules."""
    must_revalidate = []
    should_revalidate = []

    for mname, mcfg in modules.items():
        maintains = mcfg.get("maintains") or []
        ifaces = mcfg.get("interfaces") or []

        if inv_id in maintains:
            must_revalidate.append(mname)
        elif ifaces == "all" or inv_id in ifaces:
            should_revalidate.append(mname)

    if json_output:
        output = {
            "invariant": inv_id,
            "must_revalidate": must_revalidate,
            "should_revalidate": should_revalidate,
        }
        print(json.dumps(output, indent=2))
    else:
        if not quiet:
            print(f"{BOLD}Cascade analysis for {inv_id}:{RESET}")
            print()
        if must_revalidate:
            print(f"  {RED}MUST revalidate:{RESET}   {', '.join(must_revalidate)}")
        else:
            print(f"  MUST revalidate:   (none)")
        if should_revalidate:
            print(f"  {YELLOW}SHOULD revalidate:{RESET} {', '.join(should_revalidate)}")
        else:
            print(f"  SHOULD revalidate: (none)")

        if not must_revalidate and not should_revalidate:
            inv_cfg = inv_registry.get(inv_id)
            if inv_cfg is None:
                print(f"\n  {RED}WARNING: {inv_id} not found in invariant_registry{RESET}")
            else:
                print(f"\n  No modules reference {inv_id}")

    # Create beads issues if requested
    if with_beads and (must_revalidate or should_revalidate):
        import subprocess
        for mname in must_revalidate:
            title = f"[cascade:{inv_id}] Re-validate {mname} (MUST)"
            subprocess.run(["br", "create", "--title", title, "--priority", "1",
                          "--label", f"cascade:{inv_id}"], capture_output=True)
        for mname in should_revalidate:
            title = f"[cascade:{inv_id}] Re-validate {mname} (SHOULD)"
            subprocess.run(["br", "create", "--title", title, "--priority", "2",
                          "--label", f"cascade:{inv_id}"], capture_output=True)
        if not json_output and not quiet:
            total = len(must_revalidate) + len(should_revalidate)
            print(f"\n  Created {total} br issue(s) with label cascade:{inv_id}")

    sys.exit(0)


main()
PYTHON_EOF

exit $?

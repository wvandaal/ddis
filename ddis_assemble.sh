#!/usr/bin/env bash
# ddis_assemble.sh — Assembles DDIS modular spec bundles from manifest.yaml
#
# Implements §0.13.10 (Assembly Rules) of the DDIS Standard v1.0.
# Reads manifest.yaml, validates structure, and produces one bundle per module
# in the bundles/ directory.
#
# Usage:
#   ddis_assemble.sh [OPTIONS] [MODULE_NAME...]
#
# Options:
#   -m, --manifest PATH   Path to manifest.yaml (default: ./manifest.yaml)
#   -o, --output DIR      Output directory for bundles (default: ./bundles)
#   -q, --quiet           Suppress info messages (only show warnings/errors)
#   -v, --verbose         Show detailed assembly info per module
#   --dry-run             Validate manifest and show what would be assembled
#   -h, --help            Show this help
#
# If MODULE_NAME(s) given, assemble only those modules. Otherwise assemble all.
#
# Exit codes:
#   0  All bundles assembled, no budget violations
#   1  One or more bundles exceed hard ceiling (INV-014 violated)
#   2  Manifest parse error or missing files

set -euo pipefail

# --- Defaults ---
MANIFEST="./manifest.yaml"
OUTPUT_DIR="./bundles"
QUIET=false
VERBOSE=false
DRY_RUN=false
SPECIFIC_MODULES=()
EXIT_CODE=0

# --- Color output (disabled if not a terminal) ---
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    GREEN='\033[0;32m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED='' YELLOW='' GREEN='' CYAN='' BOLD='' RESET=''
fi

# --- Helpers ---
info()  { $QUIET || echo -e "${CYAN}[INFO]${RESET} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${RESET} $*" >&2; }
error() { echo -e "${RED}[ERROR]${RESET} $*" >&2; }
fatal() { error "$@"; exit 2; }

usage() {
    sed -n '2,/^$/{ s/^# //; s/^#//; p; }' "$0"
    exit 0
}

# --- Argument parsing ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        -m|--manifest) MANIFEST="$2"; shift 2 ;;
        -o|--output)   OUTPUT_DIR="$2"; shift 2 ;;
        -q|--quiet)    QUIET=true; shift ;;
        -v|--verbose)  VERBOSE=true; shift ;;
        --dry-run)     DRY_RUN=true; shift ;;
        -h|--help)     usage ;;
        -*)            fatal "Unknown option: $1" ;;
        *)             SPECIFIC_MODULES+=("$1"); shift ;;
    esac
done

# --- Validate manifest exists ---
[[ -f "$MANIFEST" ]] || fatal "Manifest not found: $MANIFEST"

# Resolve manifest directory (all paths in manifest are relative to it)
MANIFEST_DIR="$(cd "$(dirname "$MANIFEST")" && pwd)"
MANIFEST="$(cd "$(dirname "$MANIFEST")" && pwd)/$(basename "$MANIFEST")"

# --- Python assembly engine ---
# We use Python+PyYAML for robust YAML parsing and assembly logic.
# The script below implements the exact algorithm from §0.13.10.

python3 - "$MANIFEST" "$OUTPUT_DIR" "$DRY_RUN" "$QUIET" "$VERBOSE" \
    "${SPECIFIC_MODULES[@]+"${SPECIFIC_MODULES[@]}"}" <<'PYTHON_EOF'
import sys
import os
import yaml
from pathlib import Path

def main():
    args = sys.argv[1:]
    manifest_path = args[0]
    output_dir = args[1]
    dry_run = args[2].lower() == "true"
    quiet = args[3].lower() == "true"
    verbose = args[4].lower() == "true"
    specific_modules = args[5:] if len(args) > 5 else []

    # --- Parse manifest ---
    try:
        with open(manifest_path) as f:
            manifest = yaml.safe_load(f)
    except yaml.YAMLError as e:
        print(f"\033[0;31m[ERROR]\033[0m Failed to parse manifest: {e}", file=sys.stderr)
        sys.exit(2)
    except FileNotFoundError:
        print(f"\033[0;31m[ERROR]\033[0m Manifest not found: {manifest_path}", file=sys.stderr)
        sys.exit(2)

    manifest_dir = os.path.dirname(os.path.abspath(manifest_path))

    # --- Extract manifest fields ---
    tier_mode = manifest.get("tier_mode", "three-tier")
    budget = manifest.get("context_budget", {})
    target_lines = budget.get("target_lines", 4000)
    hard_ceiling = budget.get("hard_ceiling_lines", 5000)
    constitution = manifest.get("constitution", {})
    system_file = constitution.get("system")
    domains = constitution.get("domains", {}) or {}
    modules = manifest.get("modules", {}) or {}

    if not system_file:
        print("\033[0;31m[ERROR]\033[0m Manifest missing constitution.system", file=sys.stderr)
        sys.exit(2)

    # Resolve path relative to manifest directory
    def resolve(path_str):
        if path_str is None:
            return None
        p = Path(manifest_dir) / path_str
        return p

    # Validate system constitution file exists
    sys_path = resolve(system_file)
    if not sys_path.exists():
        print(f"\033[0;31m[ERROR]\033[0m System constitution not found: {sys_path}", file=sys.stderr)
        sys.exit(2)

    # Validate domain constitution files exist (three-tier only)
    if tier_mode == "three-tier":
        for domain_name, domain_cfg in domains.items():
            domain_file = domain_cfg.get("file") if isinstance(domain_cfg, dict) else domain_cfg
            dp = resolve(domain_file)
            if not dp or not dp.exists():
                print(f"\033[0;31m[ERROR]\033[0m Domain constitution not found: {dp} (domain: {domain_name})", file=sys.stderr)
                sys.exit(2)

    # Determine which modules to assemble
    if specific_modules:
        for m in specific_modules:
            if m not in modules:
                print(f"\033[0;31m[ERROR]\033[0m Module '{m}' not found in manifest. Available: {', '.join(modules.keys())}", file=sys.stderr)
                sys.exit(2)
        module_names = specific_modules
    else:
        module_names = list(modules.keys())

    if not module_names:
        print("\033[0;31m[ERROR]\033[0m No modules defined in manifest.", file=sys.stderr)
        sys.exit(2)

    # --- Create output directory ---
    if not dry_run:
        os.makedirs(os.path.join(manifest_dir, output_dir) if not os.path.isabs(output_dir) else output_dir, exist_ok=True)

    def abs_output_dir():
        if os.path.isabs(output_dir):
            return output_dir
        return os.path.join(manifest_dir, output_dir)

    # --- Read file and return contents + line count ---
    def read_file(path):
        try:
            content = path.read_text()
            lines = content.count('\n') + (1 if content and not content.endswith('\n') else 0)
            return content, lines
        except FileNotFoundError:
            print(f"\033[0;31m[ERROR]\033[0m File not found: {path}", file=sys.stderr)
            sys.exit(2)

    # --- Assembly ---
    exit_code = 0
    total_assembled = 0
    over_target_count = 0
    results = []

    if not quiet:
        mode_str = tier_mode.upper()
        print(f"\033[0;36m[INFO]\033[0m Assembling {len(module_names)} module(s) in {mode_str} mode")
        print(f"\033[0;36m[INFO]\033[0m Budget: target={target_lines}, ceiling={hard_ceiling}")

    for module_name in module_names:
        module_cfg = modules[module_name]
        module_file = module_cfg.get("file")
        module_domain = module_cfg.get("domain", "")
        deep_context = module_cfg.get("deep_context")

        # Validate module file exists
        module_path = resolve(module_file)
        if not module_path or not module_path.exists():
            print(f"\033[0;31m[ERROR]\033[0m Module file not found: {module_path} (module: {module_name})", file=sys.stderr)
            sys.exit(2)

        bundle_parts = []
        bundle_labels = []

        # --- Tier 1: System constitution (always included) ---
        content, lines = read_file(sys_path)
        bundle_parts.append(content)
        bundle_labels.append(("Tier 1 (system)", lines))

        # --- Tier 2: Domain constitution ---
        if tier_mode == "three-tier":
            if module_domain == "cross-cutting":
                # Cross-cutting modules get ALL domain constitutions
                for dname, dcfg in domains.items():
                    dfile = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
                    content, lines = read_file(resolve(dfile))
                    bundle_parts.append(content)
                    bundle_labels.append((f"Tier 2 ({dname})", lines))
            else:
                # Normal modules get their own domain constitution
                if module_domain not in domains:
                    print(f"\033[0;31m[ERROR]\033[0m Module '{module_name}' references domain '{module_domain}' "
                          f"not found in constitution.domains", file=sys.stderr)
                    sys.exit(2)
                dcfg = domains[module_domain]
                dfile = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
                content, lines = read_file(resolve(dfile))
                bundle_parts.append(content)
                bundle_labels.append((f"Tier 2 ({module_domain})", lines))

            # --- Tier 3: Cross-domain deep context (if present) ---
            if deep_context is not None:
                dp = resolve(deep_context)
                if dp.exists():
                    content, lines = read_file(dp)
                    bundle_parts.append(content)
                    bundle_labels.append(("Tier 3 (deep context)", lines))
                else:
                    print(f"\033[0;33m[WARN]\033[0m Deep context file not found: {dp} (module: {module_name})", file=sys.stderr)

        # Two-tier mode: Tier 1 has full definitions, no Tier 2 or 3

        # --- The module itself ---
        content, lines = read_file(module_path)
        bundle_parts.append(content)
        bundle_labels.append(("Module", lines))

        # --- Budget validation (INV-014) ---
        total_lines = sum(l for _, l in bundle_labels)

        status = "OK"
        if total_lines > hard_ceiling:
            status = "CEILING_EXCEEDED"
            exit_code = 1
        elif total_lines > target_lines:
            status = "OVER_TARGET"
            over_target_count += 1

        results.append({
            "name": module_name,
            "total_lines": total_lines,
            "status": status,
            "parts": bundle_labels,
        })

        # --- Output ---
        if status == "CEILING_EXCEEDED":
            print(f"\033[0;31m[ERROR]\033[0m Bundle '{module_name}': {total_lines} lines EXCEEDS "
                  f"hard ceiling {hard_ceiling}. INV-014 VIOLATED.", file=sys.stderr)
        elif status == "OVER_TARGET" and not quiet:
            print(f"\033[0;33m[WARN]\033[0m Bundle '{module_name}': {total_lines} lines exceeds "
                  f"target {target_lines} (ceiling: {hard_ceiling}).", file=sys.stderr)
        elif verbose:
            print(f"\033[0;32m[OK]\033[0m Bundle '{module_name}': {total_lines} lines")

        if verbose:
            for label, lcount in bundle_labels:
                print(f"       {label}: {lcount} lines")

        # --- Write bundle ---
        if not dry_run:
            bundle_content = "\n\n".join(bundle_parts)

            # Prepend assembly header
            header = (
                f"<!-- ASSEMBLED BUNDLE: {module_name} -->\n"
                f"<!-- Generated by ddis_assemble.sh from {os.path.basename(manifest_path)} -->\n"
                f"<!-- Tier mode: {tier_mode} | Total lines: {total_lines} | "
                f"Budget: {target_lines}/{hard_ceiling} -->\n"
                f"<!-- DO NOT EDIT — regenerate with: ddis_assemble.sh -m {os.path.basename(manifest_path)} {module_name} -->\n\n"
            )

            out_path = os.path.join(abs_output_dir(), f"{module_name}_bundle.md")
            with open(out_path, 'w') as f:
                f.write(header + bundle_content)

            total_assembled += 1

    # --- Summary ---
    if not quiet:
        print()
        if dry_run:
            print(f"\033[1m[DRY RUN]\033[0m Would assemble {len(results)} bundle(s)")
        else:
            print(f"\033[1m[DONE]\033[0m Assembled {total_assembled} bundle(s) → {abs_output_dir()}/")

        # Budget summary
        ceiling_violations = sum(1 for r in results if r["status"] == "CEILING_EXCEEDED")
        over_target = sum(1 for r in results if r["status"] == "OVER_TARGET")
        ok_count = sum(1 for r in results if r["status"] == "OK")

        if ceiling_violations:
            print(f"\033[0;31m  {ceiling_violations} bundle(s) EXCEED hard ceiling (INV-014 violated)\033[0m")
        if over_target:
            pct = (over_target / len(results)) * 100
            threshold_pct = 20
            if pct >= threshold_pct:
                print(f"\033[0;31m  {over_target}/{len(results)} ({pct:.0f}%) over target — "
                      f"Gate M-2 requires < {threshold_pct}%\033[0m")
            else:
                print(f"\033[0;33m  {over_target}/{len(results)} ({pct:.0f}%) over target "
                      f"(< {threshold_pct}% threshold: OK)\033[0m")
        if ok_count == len(results):
            print(f"\033[0;32m  All {ok_count} bundle(s) within target budget\033[0m")

    sys.exit(exit_code)

main()
PYTHON_EOF

exit $?

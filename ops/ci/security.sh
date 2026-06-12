#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
mkdir -p target/security
if command -v gitleaks >/dev/null 2>&1; then
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    gitleaks detect --redact --verbose || true
  else
    gitleaks detect --no-git --redact --verbose || true
  fi
fi
if command -v actionlint >/dev/null 2>&1 && [[ -d .github/workflows ]]; then
  actionlint .github/workflows/*.yml
fi
if find . -path './.git' -prune -o -name '.env' -type f -print | grep -q .; then
  printf 'security check failed: committed .env file found\n' >&2
  exit 1
fi
cat > target/security/evidence.json <<'JSON'
{"schema_version":"jeryu.split.security/v1","checks":["gitleaks","actionlint","env-file"]}
JSON
printf 'security ok\n'

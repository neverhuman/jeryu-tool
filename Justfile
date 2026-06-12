set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# One-command setup/validation: run the full local gate.
default:
  ./ops/ci/fast.sh
  ./ops/ci/check.sh
  ./ops/ci/score.sh
  ./ops/ci/security.sh

fast:
  ./ops/ci/fast.sh # jankurai pin drift check

check:
  ./ops/ci/check.sh

score:
  ./ops/ci/score.sh # jankurai audit repo-score

security:
  ./ops/ci/security.sh # gitleaks actionlint env-file

artifact-support:
  ./ops/ci/artifact_support.sh

profile:
  printf '%s\n' "public-portal"

#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <iterations>"
  exit 1
fi

# Get the repo root (directory containing .ralph)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RALPH_DIR="$REPO_ROOT/.ralph"

# File paths
PROMPT_FILE="$RALPH_DIR/prompt.md"
SPEC_FILE="$RALPH_DIR/spec.md"
PROGRESS_FILE="$RALPH_DIR/progress.txt"

# Verify required files exist
if [ ! -f "$PROMPT_FILE" ]; then
  echo "Error: Prompt file not found at $PROMPT_FILE"
  exit 1
fi

if [ ! -f "$SPEC_FILE" ]; then
  echo "Error: Spec file not found at $SPEC_FILE"
  exit 1
fi

# Create progress file if it doesn't exist
if [ ! -f "$PROGRESS_FILE" ]; then
  echo "# Ralph Progress" > "$PROGRESS_FILE"
  echo "" >> "$PROGRESS_FILE"
  echo "---" >> "$PROGRESS_FILE"
fi

# Load prompt and replace <repo_root> with actual path
PROMPT=$(cat "$PROMPT_FILE" | sed "s|<repo_root>|$REPO_ROOT|g")

# jq filter to extract streaming text from assistant messages
stream_text='select(.type == "assistant").message.content[]? | select(.type == "text").text // empty | gsub("\n"; "\r\n") | . + "\r\n\n"'

# jq filter to extract final result
final_result='select(.type == "result").result // empty'

for ((i=1; i<=$1; i++)); do
  echo "=== Ralph iteration $i of $1 ==="

  tmpfile=$(mktemp)
  trap "rm -f $tmpfile" EXIT

  claude \
    --dangerously-skip-permissions \
    --verbose \
    --print \
    --output-format stream-json \
    "$PROMPT" \
  | grep --line-buffered '^{' \
  | tee "$tmpfile" \
  | jq --unbuffered -rj "$stream_text"

  result=$(jq -r "$final_result" "$tmpfile")

  if [[ "$result" == *"<promise>COMPLETE</promise>"* ]]; then
    echo ""
    echo "=== Ralph complete after $i iterations ==="
    exit 0
  fi
done

echo ""
echo "=== Ralph finished $1 iterations (spec may not be complete) ==="

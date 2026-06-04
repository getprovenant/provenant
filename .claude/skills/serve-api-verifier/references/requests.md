# Serve API Request Examples

These examples assume:

```bash
SERVE_BASE_URL="http://127.0.0.1:8080"
```

## Health

```bash
curl -sS "${SERVE_BASE_URL}/livez"
curl -sS "${SERVE_BASE_URL}/readyz"
curl -sS "${SERVE_BASE_URL}/version"
```

Assert response fields with `jq` when available:

```bash
curl -sS "${SERVE_BASE_URL}/livez" | jq -e '.status == "ok"'
curl -sS "${SERVE_BASE_URL}/readyz" | jq -e '.status == "ready" and .api_version == "v1"'
curl -sS "${SERVE_BASE_URL}/version" | jq -e '.service == "provenant-serve" and .api_version == "v1"'
```

## Synchronous Local-Path Scan

```bash
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
  -H 'Content-Type: application/json' \
  -d '{
    "input": { "type": "paths", "paths": ["/absolute/path/to/repo"] },
    "options": {
      "detect_license": { "type": "embedded" },
      "detect_packages": true
    }
  }'
```

## Asynchronous URL Scan

```bash
response="$(
  curl -sS \
    -X POST "${SERVE_BASE_URL}/v1/scans:async" \
    -H 'Content-Type: application/json' \
    -d '{
      "input": {
        "type": "url",
        "url": "https://github.com/aboutcode-org/scancode.io/archive/refs/heads/main.zip"
      },
      "options": { "detect_packages": true }
    }'
)"
job_id="$(printf '%s' "$response" | jq -r '.job_id')"
curl -sS "${SERVE_BASE_URL}/v1/jobs/${job_id}"
curl -sS "${SERVE_BASE_URL}/v1/jobs/${job_id}/result"
```

Do not stop after the accepted response. Check the job state and final result.

## Upload Input

```bash
CONTENT_BASE64="$(base64 < snapshot.zip | tr -d '\n')"
curl -sS \
  -X POST "${SERVE_BASE_URL}/v1/scans" \
  -H 'Content-Type: application/json' \
  --data-binary @- <<EOF
{
  "input": {
    "type": "upload",
    "filename": "snapshot.zip",
    "content_base64": "${CONTENT_BASE64}"
  },
  "options": { "detect_license": { "type": "embedded" } }
}
EOF
```

# Template Dockerfile for a new ClawEval matrix agent.
# Copy this directory, rename it, and customize.
#
# Steps:
#   1. Replace the agent installation section below with your agent's build/install.
#   2. Update config.json with the correct command, args, and mode.
#   3. Add a service entry in ../docker-compose.yml.
#   4. Add the agent to ../matrix.json.

# --- Stage 1: Build your agent (if needed) ---
# FROM golang:1.22-alpine AS builder
# RUN go install github.com/your/agent@latest

# --- Stage 2: Runtime ---
FROM python:3.11-slim

# Copy agent binary from builder (uncomment if using multi-stage)
# COPY --from=builder /go/bin/my-agent /usr/local/bin/my-agent

# Install adapter dependencies
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Copy adapter and agent config
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/_template/config.json /app/config.json

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000

EXPOSE 5000

CMD ["python", "/app/adapter.py"]

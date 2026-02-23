FROM golang:1.26-alpine AS builder

RUN apk add --no-cache git make

WORKDIR /src
RUN git clone --depth 1 https://github.com/sipeed/picoclaw.git .
RUN go mod download
RUN make build

FROM python:3.11-slim

# Copy picoclaw binary
COPY --from=builder /src/build/picoclaw /usr/local/bin/picoclaw

# Install adapter dependencies
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Initialize picoclaw workspace
RUN mkdir -p /root/.picoclaw/workspace/sessions

# Bake in picoclaw config (API key injected at runtime via entrypoint)
COPY matrix/agents/picoclaw/picoclaw_config.json /root/.picoclaw/config.json

# Copy adapter, agent config, wrapper script, and entrypoint
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/picoclaw/config.json /app/config.json
COPY matrix/agents/picoclaw/picoclaw_run.sh /app/picoclaw_run.sh
COPY matrix/agents/picoclaw/entrypoint.sh /app/entrypoint.sh
RUN sed -i 's/\r$//' /app/entrypoint.sh /app/picoclaw_run.sh && \
    chmod +x /app/entrypoint.sh /app/picoclaw_run.sh

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000
ENV NO_COLOR=1

EXPOSE 5000

CMD ["/app/entrypoint.sh"]

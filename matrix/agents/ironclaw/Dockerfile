# IronClaw agent — uses pre-built binary in single-message CLI mode.

FROM python:3.11-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends curl ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Install ironclaw from pre-built binary
RUN curl --proto '=https' --tlsv1.2 -LsSf \
    https://github.com/nearai/ironclaw/releases/latest/download/ironclaw-installer.sh | sh && \
    cp /root/.cargo/bin/ironclaw /usr/local/bin/ironclaw

# Install adapter dependencies
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Copy adapter, agent config, and wrapper script
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/ironclaw/config.json /app/config.json
COPY matrix/agents/ironclaw/ironclaw_run.sh /app/ironclaw_run.sh
RUN sed -i 's/\r$//' /app/ironclaw_run.sh && chmod +x /app/ironclaw_run.sh

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000
ENV NO_COLOR=1

EXPOSE 5000

CMD ["python", "/app/adapter.py"]

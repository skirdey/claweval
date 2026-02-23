FROM python:3.11-slim

# Install Node.js 22 (required by openclaw)
RUN apt-get update && \
    apt-get install -y --no-install-recommends curl ca-certificates gnupg git && \
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y --no-install-recommends nodejs && \
    rm -rf /var/lib/apt/lists/*

# Install openclaw globally
RUN npm install -g openclaw@latest

# Install adapter dependencies
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Copy adapter and agent config
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/openclaw/config.json /app/config.json

# Pre-create openclaw config/state directory and set model to use OpenRouter
RUN mkdir -p /root/.openclaw && \
    openclaw models set openrouter/anthropic/claude-opus-4-6

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000
ENV NO_COLOR=1

EXPOSE 5000

CMD ["python", "/app/adapter.py"]

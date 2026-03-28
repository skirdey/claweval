FROM python:3.11-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    git ca-certificates curl build-essential && \
    rm -rf /var/lib/apt/lists/*

# Install hermes-agent from source
RUN pip install --no-cache-dir "hermes-agent @ git+https://github.com/NousResearch/hermes-agent.git"

# Install adapter
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/hermes/config.json /app/config.json
COPY matrix/agents/hermes/hermes_run.sh /app/hermes_run.sh
COPY matrix/agents/hermes/entrypoint.sh /app/entrypoint.sh
RUN sed -i 's/\r$//' /app/entrypoint.sh /app/hermes_run.sh && \
    chmod +x /app/entrypoint.sh /app/hermes_run.sh

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000
ENV NO_COLOR=1
ENV HERMES_HOME=/root/.hermes
ENV HERMES_MODEL=qwen/qwen3.5-397b-a17b

EXPOSE 5000
CMD ["/app/entrypoint.sh"]

FROM python:3.12-slim

# Install nanobot and adapter dependencies
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt && \
    pip install --no-cache-dir nanobot-ai

# Initialize nanobot workspace directories
RUN mkdir -p /root/.nanobot/workspace/sessions \
             /root/.nanobot/workspace/memory

# Bake in nanobot config (API key injected at runtime via entrypoint)
COPY matrix/agents/nanobot/nanobot_config.json /root/.nanobot/config.json

# Copy adapter, agent config, wrapper script, and entrypoint
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/nanobot/config.json /app/config.json
COPY matrix/agents/nanobot/nanobot_run.sh /app/nanobot_run.sh
COPY matrix/agents/nanobot/entrypoint.sh /app/entrypoint.sh
RUN sed -i 's/\r$//' /app/entrypoint.sh /app/nanobot_run.sh && \
    chmod +x /app/entrypoint.sh /app/nanobot_run.sh

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000
ENV NO_COLOR=1

EXPOSE 5000

CMD ["/app/entrypoint.sh"]

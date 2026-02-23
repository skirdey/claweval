FROM python:3.11-slim

# Install adapter dependencies (no agent binary needed — pure API proxy)
COPY matrix/adapter/requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r /app/requirements.txt

# Copy adapter and agent config
COPY matrix/adapter/adapter.py /app/adapter.py
COPY matrix/agents/openai_direct/config.json /app/config.json

ENV ADAPTER_CONFIG=/app/config.json
ENV PORT=5000

EXPOSE 5000

CMD ["python", "/app/adapter.py"]

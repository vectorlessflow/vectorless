# Custom Configuration Example

Use your own API key, model, and endpoint.

## Setup

```bash
pip install vectorless
```

## Configure

Edit `main.py` and update the settings:

```python
API_KEY = "your-api-key"
MODEL = "gpt-4o-mini"  # or "deepseek-chat", "claude-3-5-sonnet", etc.
ENDPOINT = "https://api.openai.com/v1"  # or your custom endpoint
```

## Run

```bash
python main.py
```

## Other Providers

### DeepSeek
```python
API_KEY = "sk-..."
MODEL = "deepseek-chat"
ENDPOINT = "https://api.deepseek.com/v1"
```

### Azure OpenAI
```python
API_KEY = "your-azure-key"
MODEL = "gpt-4o"
ENDPOINT = "https://your-resource.openai.azure.com/openai/deployments/your-deployment"
```

### Local LLM (Ollama)
```python
API_KEY = None  # Not needed
MODEL = "llama3"
ENDPOINT = "http://localhost:11434/v1"
```

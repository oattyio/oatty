# VS Code Security: Safely Storing Bearer Tokens

This guide explains how to safely store bearer tokens for use in VS Code launch configurations without exposing them in version control.

## 🔐 Recommended Method: Environment Variables

### 1. Create a `.env` file (NOT committed to git)

Create a `.env` file in your project root:

```bash
# Copy the example file
cp .env.example .env

# Edit with your actual tokens
nano .env
```

Your `.env` file should contain:
```bash
# Heroku API Key (Bearer Token)
HEROKU_API_KEY=your_actual_heroku_api_key_here

# Other API tokens you might need
# API_TOKEN=your_other_token_here
```

### 2. VS Code Launch Configuration

Your launch configurations now include `"envFile": "${workspaceFolder}/.env"` which automatically loads environment variables from the `.env` file.

### 3. Verify `.gitignore` Protection

Ensure your `.gitignore` includes:
```gitignore
# Environment files with secrets
.env
.env.local
.env.*.local
```

## 🛡️ Alternative Methods

### Option B: System Environment Variables

Set environment variables in your shell profile:

**macOS/Linux** (`~/.zshrc` or `~/.bashrc`):
```bash
export HEROKU_API_KEY="your_token_here"
```

**Windows** (System Properties → Environment Variables):
- Add `HEROKU_API_KEY` with your token value

### Option C: VS Code User Settings (Less Secure)

In VS Code settings (`settings.json`):
```json
{
    "terminal.integrated.env.osx": {
        "HEROKU_API_KEY": "your_token_here"
    }
}
```

⚠️ **Warning**: This method stores tokens in VS Code settings which may be synced across devices.

## 🔍 Verification

To verify your token is working:

1. Create your `.env` file with your actual token
2. Run the launch configuration "CodeLLDB: heroku-cli (CLI)"
3. Check that the API calls work without authentication errors

## 🚨 Security Best Practices

1. **Never commit tokens to version control**
2. **Use `.env` files for local development**
3. **Rotate tokens regularly**
4. **Use different tokens for different environments**
5. **Enable token expiration when possible**
6. **Monitor token usage for suspicious activity**

## 🔧 Troubleshooting

### Token not found errors:
- Verify `.env` file exists and has correct format
- Check that `HEROKU_API_KEY` is set correctly
- Ensure no extra spaces or quotes around the token

### Permission errors:
- Verify token has correct permissions
- Check if token is expired
- Ensure token is for the correct environment (staging vs production)

## 📝 Example Usage

With the current setup, your Rust code can access the token via:

```rust
let token = std::env::var("HEROKU_API_KEY")
    .expect("HEROKU_API_KEY must be set");
```

The `heroku-api` crate already handles this pattern in `HerokuClient::new_from_env()`.

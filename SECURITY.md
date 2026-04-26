# Security Policy

## Reporting a Vulnerability

We take the security of sysman seriously. If you believe you have found a security vulnerability, please report it to us as described below.

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email at: [INSERT YOUR EMAIL HERE]

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

After the initial reply to your report, we will send you a more detailed response indicating the next steps in handling your report. After the initial reply, we will keep you informed of the progress toward a fix and full announcement, and may ask for additional information or guidance.

## Preferred Languages

We prefer all communications to be in English.

## Security Best Practices

If you are contributing to sysman, please follow these security best practices:

1. **Never commit secrets**: API keys, passwords, tokens, or credentials should never be committed to the repository.
2. **Validate external input**: All user input, file contents, and external data should be validated before use.
3. **Use safe Rust patterns**: Avoid `unsafe` blocks unless absolutely necessary and well-justified.
4. **Keep dependencies updated**: Regularly audit and update dependencies for security patches.

## Known Limitations

- sysman runs with the permissions of the user who launches it. Running as root gives the tool full system access.
- Network tools (ping, traceroute, DNS lookup) invoke external system binaries.
- Service management requires systemd and appropriate Linux permissions.
